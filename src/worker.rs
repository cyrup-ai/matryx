//! # Async Matrix Client Worker
//!
//! The worker thread handles asynchronous work, and can receive messages from the main thread that
//! block on a reply from the async worker.
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::{Debug, Formatter};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::{stream::FuturesUnordered, StreamExt};
use gethostname::gethostname;
use matrix_sdk::ruma::events::AnySyncTimelineEvent;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tracing::{error, warn};
use url::Url;

use matrix_sdk::{
    authentication::matrix::MatrixSession,
    config::{RequestConfig, SyncSettings},
    deserialized_responses::DisplayName,
    encryption::verification::{SasVerification, Verification},
    encryption::{BackupDownloadStrategy, EncryptionSettings},
    event_handler::Ctx,
    reqwest,
    room::{Messages, MessagesOptions, Room as MatrixRoom, RoomMember},
    ruma::{
        api::client::{
            filter::{FilterDefinition, LazyLoadOptions, RoomEventFilter, RoomFilter},
            room::create_room::v3::{CreationContent, Request as CreateRoomRequest, RoomPreset},
            room::Visibility,
            space::get_hierarchy::v1::Request as SpaceHierarchyRequest,
        },
        assign,
        events::{
            key::verification::{
                done::{OriginalSyncKeyVerificationDoneEvent, ToDeviceKeyVerificationDoneEvent},
                key::{OriginalSyncKeyVerificationKeyEvent, ToDeviceKeyVerificationKeyEvent},
                request::ToDeviceKeyVerificationRequestEvent,
                start::{OriginalSyncKeyVerificationStartEvent, ToDeviceKeyVerificationStartEvent},
                VerificationMethod,
            },
            presence::PresenceEvent,
            reaction::ReactionEventContent,
            receipt::ReceiptType,
            receipt::{ReceiptEventContent, ReceiptThread},
            room::{
                encryption::RoomEncryptionEventContent,
                member::OriginalSyncRoomMemberEvent,
                message::{MessageType, RoomMessageEventContent},
                name::RoomNameEventContent,
                redaction::OriginalSyncRoomRedactionEvent,
            },
            tag::Tags,
            typing::SyncTypingEvent,
            AnyInitialStateEvent,
            AnyMessageLikeEvent,
            EmptyStateKey,
            InitialStateEvent,
            SyncEphemeralRoomEvent,
            SyncMessageLikeEvent,
            SyncStateEvent,
        },
        room::RoomType,
        serde::Raw,
        EventEncryptionAlgorithm,
        EventId,
        OwnedEventId,
        OwnedRoomId,
        OwnedRoomOrAliasId,
        OwnedUserId,
        RoomId,
        RoomVersionId,
    },
    Client,
    ClientBuildError,
    Error as MatrixError,
    RoomDisplayName,
    RoomMemberships,
};

// Import the Matrix SDK wrappers
use cyrum_matrix::client::CyrumClient;
use cyrum_matrix::room::CyrumRoom;
use cyrum_matrix::sync::CyrumSync;
use cyrum_matrix::encryption::CyrumEncryption;
use cyrum_matrix::error::Result as MatrixResult;
use cyrum_matrix::future::{MatrixFuture, MatrixStream};

use modalkit::errors::UIError;
use modalkit::prelude::{EditInfo, InfoMessage};

use crate::base::Need;
use crate::notifications::register_notifications;
use crate::{
    base::{
        AsyncProgramStore,
        ChatStore,
        CreateRoomFlags,
        CreateRoomType,
        IambError,
        IambResult,
        ProgramStore,
        RoomFetchStatus,
        RoomInfo,
        VerifyAction,
    },
    ApplicationSettings,
};

const DEFAULT_ENCRYPTION_SETTINGS: EncryptionSettings = EncryptionSettings {
    auto_enable_cross_signing: true,
    auto_enable_backups: true,
    backup_download_strategy: BackupDownloadStrategy::AfterDecryptionFailure,
};

const IAMB_DEVICE_NAME: &str = "iamb";
const IAMB_USER_AGENT: &str = "iamb";
const MIN_MSG_LOAD: u32 = 50;

type MessageFetchResult =
    IambResult<(Option<String>, Vec<(AnyMessageLikeEvent, Vec<OwnedUserId>)>)>;

fn initial_devname() -> String {
    format!("{} on {}", IAMB_DEVICE_NAME, gethostname().to_string_lossy())
}

async fn is_direct(room: &MatrixRoom) -> bool {
    room.deref().is_direct().await.unwrap_or_default()
}

pub async fn create_room(
    client: &Client,
    room_alias_name: Option<String>,
    rt: CreateRoomType,
    flags: CreateRoomFlags,
) -> IambResult<OwnedRoomId> {
    let mut creation_content = None;
    let mut initial_state = vec![];
    let mut is_direct = false;
    let mut preset = None;
    let mut invite = vec![];

    let visibility = if flags.contains(CreateRoomFlags::PUBLIC) {
        Visibility::Public
    } else {
        Visibility::Private
    };

    match rt {
        CreateRoomType::Direct(user) => {
            invite.push(user);
            is_direct = true;
            preset = Some(RoomPreset::TrustedPrivateChat);
        },
        CreateRoomType::Space => {
            let mut cc = CreationContent::new();
            cc.room_type = Some(RoomType::Space);

            let raw_cc = Raw::new(&cc).map_err(IambError::from)?;
            creation_content = Some(raw_cc);
        },
        CreateRoomType::Room => {},
    }

    // Set up encryption.
    if flags.contains(CreateRoomFlags::ENCRYPTED) {
        // XXX: Once matrix-sdk uses ruma 0.8, then this can skip the cast.
        let algo = EventEncryptionAlgorithm::MegolmV1AesSha2;
        let content = RoomEncryptionEventContent::new(algo);
        let encr = InitialStateEvent { content, state_key: EmptyStateKey };
        let encr_raw = Raw::new(&encr).map_err(IambError::from)?;
        let encr_raw = encr_raw.cast::<AnyInitialStateEvent>();
        initial_state.push(encr_raw);
    }

    let request = assign!(CreateRoomRequest::new(), {
        room_alias_name,
        creation_content,
        initial_state,
        invite,
        is_direct,
        visibility,
        preset,
    });

    let resp = client.create_room(request).await.map_err(IambError::from)?;

    if is_direct {
        if let Some(room) = client.get_room(resp.room_id()) {
            room.set_is_direct(true).await.map_err(IambError::from)?;
        } else {
            error!(
                room_id = resp.room_id().as_str(),
                "Couldn't set is_direct for new direct message room"
            );
        }
    }

    return Ok(resp.room_id().to_owned());
}

async fn update_event_receipts(info: &mut RoomInfo, room: &MatrixRoom, event_id: &EventId) {
    let receipts = match room
        .load_event_receipts(ReceiptType::Read, ReceiptThread::Main, event_id)
        .await
    {
        Ok(receipts) => receipts,
        Err(e) => {
            tracing::warn!(?event_id, "failed to get event receipts: {e}");
            return;
        },
    };

    for (user_id, _) in receipts {
        info.set_receipt(user_id, event_id.to_owned());
    }
}

#[derive(Debug)]
enum Plan {
    Messages(OwnedRoomId, Option<String>),
    Members(OwnedRoomId),
}

async fn load_plans(store: &AsyncProgramStore) -> Vec<Plan> {
    let mut locked = store.lock().await;
    let ChatStore { need_load, rooms, .. } = &mut locked.application;
    let mut plan = Vec::with_capacity(need_load.rooms() * 2);

    for (room_id, mut need) in std::mem::take(need_load).into_iter() {
        if need.contains(Need::MESSAGES) {
            let info = rooms.get_or_default(room_id.clone());

            if !info.recently_fetched() && !info.fetching {
                info.fetch_last = Instant::now().into();
                info.fetching = true;

                let fetch_id = match &info.fetch_id {
                    RoomFetchStatus::Done => continue,
                    RoomFetchStatus::HaveMore(fetch_id) => Some(fetch_id.clone()),
                    RoomFetchStatus::NotStarted => None,
                };

                plan.push(Plan::Messages(room_id.to_owned(), fetch_id));
                need.remove(Need::MESSAGES);
            }
        }
        if need.contains(Need::MEMBERS) {
            plan.push(Plan::Members(room_id.to_owned()));
            need.remove(Need::MEMBERS);
        }
        if !need.is_empty() {
            need_load.insert(room_id, need);
        }
    }

    return plan;
}

async fn run_plan(client: &CyrumClient, store: &AsyncProgramStore, plan: Plan, permits: &Semaphore) {
    let permit = permits.acquire().await;
    match plan {
        Plan::Messages(room_id, fetch_id) => {
            let limit = MIN_MSG_LOAD;
            let client = client.clone();
            let store_clone = store.clone();

            let res = load_older_one(&client, &room_id, fetch_id, limit).await;
            let mut locked = store.lock().await;
            load_insert(room_id, res, locked.deref_mut(), store_clone);
        },
        Plan::Members(room_id) => {
            let res = members_load(client, &room_id).await;
            let mut locked = store.lock().await;
            members_insert(room_id, res, locked.deref_mut());
        },
    }
    drop(permit);
}

async fn load_older_one(
    client: &CyrumClient,
    room_id: &RoomId,
    fetch_id: Option<String>,
    limit: u32,
) -> MessageFetchResult {
    // For historical message loading, we need to continue using the matrix-sdk types
    // directly as CyrumRoom doesn't yet have full support for all the options we need
    if let Some(cyrum_room) = client.get_room(room_id) {
        let matrix_room = cyrum_room.inner();
        
        let mut opts = match &fetch_id {
            Some(id) => MessagesOptions::backward().from(id.as_str()),
            None => MessagesOptions::backward(),
        };
        opts.limit = limit.into();

        let Messages { end, chunk, .. } = matrix_room.messages(opts).await.map_err(IambError::from)?;

        let mut msgs = vec![];

        for ev in chunk.into_iter() {
            let deserialized = ev.into_raw().deserialize().map_err(IambError::Serde)?;
            let msg: AnyMessageLikeEvent = match deserialized {
                AnySyncTimelineEvent::MessageLike(e) => e.into_full_event(room_id.to_owned()),
                AnySyncTimelineEvent::State(_) => continue,
            };

            let event_id = msg.event_id();
            let receipts = match matrix_room
                .load_event_receipts(ReceiptType::Read, ReceiptThread::Main, event_id)
                .await
            {
                Ok(receipts) => receipts.into_iter().map(|(u, _)| u).collect(),
                Err(e) => {
                    tracing::warn!(?event_id, "failed to get event receipts: {e}");
                    vec![]
                },
            };

            msgs.push((msg, receipts));
        }

        Ok((end, msgs))
    } else {
        Err(IambError::UnknownRoom(room_id.to_owned()).into())
    }
}

fn load_insert(
    room_id: OwnedRoomId,
    res: MessageFetchResult,
    locked: &mut ProgramStore,
    store: AsyncProgramStore,
) {
    let ChatStore { presences, rooms, worker, picker, settings, .. } = &mut locked.application;
    let info = rooms.get_or_default(room_id.clone());
    info.fetching = false;
    let client = &worker.client;

    match res {
        Ok((fetch_id, msgs)) => {
            for (msg, receipts) in msgs.into_iter() {
                let sender = msg.sender().to_owned();
                let _ = presences.get_or_default(sender);

                for user_id in receipts {
                    info.set_receipt(user_id, msg.event_id().to_owned());
                }

                match msg {
                    AnyMessageLikeEvent::RoomEncrypted(msg) => {
                        info.insert_encrypted(msg);
                    },
                    AnyMessageLikeEvent::RoomMessage(msg) => {
                        info.insert_with_preview(
                            room_id.clone(),
                            store.clone(),
                            *picker,
                            msg,
                            settings,
                            client.media(),
                        );
                    },
                    AnyMessageLikeEvent::Reaction(ev) => {
                        info.insert_reaction(ev);
                    },
                    _ => continue,
                }
            }

            info.fetch_id = fetch_id.map_or(RoomFetchStatus::Done, RoomFetchStatus::HaveMore);
        },
        Err(e) => {
            warn!(room_id = room_id.as_str(), err = e.to_string(), "Failed to load older messages");

            // Wait and try again.
            locked.application.need_load.insert(room_id, Need::MESSAGES);
        },
    }
}

async fn load_older(client: &CyrumClient, store: &AsyncProgramStore) -> usize {
    // This is an arbitrary limit on how much work we do in parallel to avoid
    // spawning too many tasks at startup and overwhelming the client. We
    // should normally only surpass this limit at startup when doing an initial.
    // fetch for each room.
    const LIMIT: usize = 15;

    // Plans are run in parallel. Any room *may* have several plans.
    let plans = load_plans(store).await;
    let permits = Semaphore::new(LIMIT);

    plans
        .into_iter()
        .map(|plan| run_plan(client, store, plan, &permits))
        .collect::<FuturesUnordered<_>>()
        .count()
        .await
}

async fn members_load(client: &CyrumClient, room_id: &RoomId) -> IambResult<Vec<RoomMember>> {
    if let Some(room) = client.get_room(room_id) {
        // Get members using CyrumRoom's members method
        match room.members().await {
            Ok(cyrum_members) => {
                // Convert CyrumRoomMember to RoomMember for compatibility
                let matrix_members: Vec<RoomMember> = cyrum_members.into_iter()
                    .map(|m| m.inner().clone())
                    .collect();
                
                Ok(matrix_members)
            },
            Err(e) => {
                Err(IambError::MatrixSdk(e.to_string()).into())
            }
        }
    } else {
        Err(IambError::UnknownRoom(room_id.to_owned()).into())
    }
}

fn members_insert(
    room_id: OwnedRoomId,
    res: IambResult<Vec<RoomMember>>,
    store: &mut ProgramStore,
) {
    if let Ok(members) = res {
        let ChatStore { rooms, .. } = &mut store.application;
        let info = rooms.get_or_default(room_id);

        for member in members {
            let user_id = member.user_id();
            let display_name =
                member.display_name().map_or(user_id.to_string(), |str| str.to_string());
            info.display_names.insert(user_id.to_owned(), display_name);
        }
    }
    // else ???
}

async fn load_older_forever(client: &CyrumClient, store: &AsyncProgramStore) {
    // Load any pending older messages or members every 2 seconds.
    let mut interval = tokio::time::interval(Duration::from_secs(2));

    loop {
        interval.tick().await;
        load_older(client, store).await;
    }
}

async fn refresh_rooms(client: &CyrumClient, store: &AsyncProgramStore) {
    let mut names = vec![];

    let mut spaces = vec![];
    let mut rooms = vec![];
    let mut dms = vec![];

    // Process joined rooms using CyrumClient
    for cyrum_room in client.joined_rooms() {
        let room_id = cyrum_room.room_id().to_owned();
        let name = cyrum_room.name().unwrap_or_else(|| room_id.to_string());
        
        // Get tags - for now, we have to use the inner matrix room for compatibility
        let tags = cyrum_room.inner().tags().await.unwrap_or_default();
        
        // Note the room name
        names.push((room_id.clone(), name));
        
        // Add to the appropriate list
        let matrix_room = cyrum_room.inner().clone();
        
        if cyrum_room.is_direct() {
            dms.push(Arc::new((matrix_room, tags)));
        } else if cyrum_room.inner().is_space() {
            spaces.push(Arc::new((matrix_room, tags)));
        } else {
            rooms.push(Arc::new((matrix_room, tags)));
        }
    }
    
    // For invited rooms, we still need to use the matrix-sdk client for now
    // as CyrumClient doesn't have invited_rooms method yet
    for room in client.inner().invited_rooms().into_iter() {
        let name = room.cached_display_name().unwrap_or(RoomDisplayName::Empty).to_string();
        let tags = room.tags().await.unwrap_or_default();

        names.push((room.room_id().to_owned(), name));

        if is_direct(&room).await {
            dms.push(Arc::new((room, tags)));
        } else if room.is_space() {
            spaces.push(Arc::new((room, tags)));
        } else {
            rooms.push(Arc::new((room, tags)));
        }
    }

    let mut locked = store.lock().await;
    locked.application.sync_info.spaces = spaces;
    locked.application.sync_info.rooms = rooms;
    locked.application.sync_info.dms = dms;

    for (room_id, name) in names {
        locked.application.set_room_name(&room_id, &name);
    }
}

async fn refresh_rooms_forever(client: &CyrumClient, store: &AsyncProgramStore) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));

    loop {
        refresh_rooms(client, store).await;
        interval.tick().await;
    }
}

async fn send_receipts_forever(client: &CyrumClient, store: &AsyncProgramStore) {
    let mut interval = tokio::time::interval(Duration::from_secs(2));
    let mut sent = HashMap::<OwnedRoomId, OwnedEventId>::default();

    loop {
        interval.tick().await;

        let locked = store.lock().await;
        let user_id = &locked.application.settings.profile.user_id;
        let updates = client
            .joined_rooms()
            .into_iter()
            .filter_map(|room| {
                let room_id = room.room_id().to_owned();
                let info = locked.application.rooms.get(&room_id)?;
                let new_receipt = info.get_receipt(user_id)?;
                let old_receipt = sent.get(&room_id);
                if Some(new_receipt) != old_receipt {
                    Some((room_id, new_receipt.clone()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        drop(locked);

        for (room_id, new_receipt) in updates {
            if let Some(room) = client.get_room(room_id.as_ref()) {
                // CyrumRoom doesn't have a direct send_single_receipt method,
                // so we need to use mark_as_read which effectively does the same thing
                match room.mark_as_read(&new_receipt).await {
                    Ok(_) => {
                        sent.insert(room_id, new_receipt);
                    },
                    Err(e) => tracing::warn!(?room_id, "Failed to set read receipt: {}", e),
                }
            }
        }
    }
}

pub async fn do_first_sync(client: &CyrumClient, store: &AsyncProgramStore) -> Result<(), MatrixError> {
    // Get CyrumSync from the client
    let sync = client.sync();
    
    // Perform an initial sync - CyrumSync handles the details internally
    sync.sync_once().await
        .map_err(|e| MatrixError::UnknownError(e.to_string()))?;
    
    // Populate sync_info with our initial set of rooms/dms/spaces.
    refresh_rooms(client, store).await;

    // Insert Need::Messages to fetch accurate recent timestamps in the background.
    let mut locked = store.lock().await;
    let ChatStore { sync_info, need_load, .. } = &mut locked.application;

    for room in sync_info.rooms.iter() {
        let room_id = room.as_ref().0.room_id().to_owned();
        need_load.insert(room_id, Need::MESSAGES);
    }

    for room in sync_info.dms.iter() {
        let room_id = room.as_ref().0.room_id().to_owned();
        need_load.insert(room_id, Need::MESSAGES);
    }

    Ok(())
}

#[derive(Debug)]
pub enum LoginStyle {
    SessionRestore(MatrixSession),
    Password(String),
    SingleSignOn,
}

pub struct ClientResponse<T>(Receiver<T>);
pub struct ClientReply<T>(SyncSender<T>);

impl<T> ClientResponse<T> {
    fn recv(self) -> T {
        self.0.recv().expect("failed to receive response from client thread")
    }
}

impl<T> ClientReply<T> {
    fn send(self, t: T) {
        self.0.send(t).unwrap();
    }
}

fn oneshot<T>() -> (ClientReply<T>, ClientResponse<T>) {
    let (tx, rx) = sync_channel(1);
    let reply = ClientReply(tx);
    let response = ClientResponse(rx);

    return (reply, response);
}

pub type FetchedRoom = (MatrixRoom, RoomDisplayName, Option<Tags>);

pub enum WorkerTask {
    Init(AsyncProgramStore, ClientReply<()>),
    Login(LoginStyle, ClientReply<IambResult<EditInfo>>),
    Logout(String, ClientReply<IambResult<EditInfo>>),
    GetInviter(MatrixRoom, ClientReply<IambResult<Option<RoomMember>>>),
    GetRoom(OwnedRoomId, ClientReply<IambResult<FetchedRoom>>),
    JoinRoom(String, ClientReply<IambResult<OwnedRoomId>>),
    Members(OwnedRoomId, ClientReply<IambResult<Vec<RoomMember>>>),
    SpaceMembers(OwnedRoomId, ClientReply<IambResult<Vec<OwnedRoomId>>>),
    TypingNotice(OwnedRoomId),
    Verify(VerifyAction, SasVerification, ClientReply<IambResult<EditInfo>>),
    VerifyRequest(OwnedUserId, ClientReply<IambResult<EditInfo>>),
}

impl Debug for WorkerTask {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            WorkerTask::Init(_, _) => {
                f.debug_tuple("WorkerTask::Init")
                    .field(&format_args!("_"))
                    .field(&format_args!("_"))
                    .finish()
            },
            WorkerTask::Login(style, _) => {
                f.debug_tuple("WorkerTask::Login")
                    .field(style)
                    .field(&format_args!("_"))
                    .finish()
            },
            WorkerTask::Logout(user_id, _) => {
                f.debug_tuple("WorkerTask::Logout").field(user_id).finish()
            },
            WorkerTask::GetInviter(invite, _) => {
                f.debug_tuple("WorkerTask::GetInviter").field(invite).finish()
            },
            WorkerTask::GetRoom(room_id, _) => {
                f.debug_tuple("WorkerTask::GetRoom")
                    .field(room_id)
                    .field(&format_args!("_"))
                    .finish()
            },
            WorkerTask::JoinRoom(s, _) => {
                f.debug_tuple("WorkerTask::JoinRoom")
                    .field(s)
                    .field(&format_args!("_"))
                    .finish()
            },
            WorkerTask::Members(room_id, _) => {
                f.debug_tuple("WorkerTask::Members")
                    .field(room_id)
                    .field(&format_args!("_"))
                    .finish()
            },
            WorkerTask::SpaceMembers(room_id, _) => {
                f.debug_tuple("WorkerTask::SpaceMembers")
                    .field(room_id)
                    .field(&format_args!("_"))
                    .finish()
            },
            WorkerTask::TypingNotice(room_id) => {
                f.debug_tuple("WorkerTask::TypingNotice").field(room_id).finish()
            },
            WorkerTask::Verify(act, sasv1, _) => {
                f.debug_tuple("WorkerTask::Verify")
                    .field(act)
                    .field(sasv1)
                    .field(&format_args!("_"))
                    .finish()
            },
            WorkerTask::VerifyRequest(user_id, _) => {
                f.debug_tuple("WorkerTask::VerifyRequest")
                    .field(user_id)
                    .field(&format_args!("_"))
                    .finish()
            },
        }
    }
}

async fn create_client_inner(
    homeserver: &Option<Url>,
    settings: &ApplicationSettings,
) -> Result<Client, ClientBuildError> {
    let req_timeout = Duration::from_secs(settings.tunables.request_timeout);

    // Set up the HTTP client.
    let http = reqwest::Client::builder()
        .user_agent(IAMB_USER_AGENT)
        .timeout(req_timeout)
        .pool_idle_timeout(Duration::from_secs(60))
        .pool_max_idle_per_host(10)
        .tcp_keepalive(Duration::from_secs(10))
        .build()
        .unwrap();

    let req_config = RequestConfig::new().timeout(req_timeout).retry_timeout(req_timeout);

    // Set up the Matrix client for the selected profile.
    let builder = Client::builder()
        .http_client(http)
        .sqlite_store(settings.sqlite_dir.as_path(), None)
        .request_config(req_config)
        .with_encryption_settings(DEFAULT_ENCRYPTION_SETTINGS);

    let builder = if let Some(url) = homeserver {
        // Use the explicitly specified homeserver.
        builder.homeserver_url(url.as_str())
    } else {
        // Try to discover the homeserver from the user ID.
        let account = &settings.profile;
        builder.server_name(account.user_id.server_name())
    };

    builder.build().await
}

pub async fn create_client(settings: &ApplicationSettings) -> Client {
    let account = &settings.profile;
    let res = match create_client_inner(&account.url, settings).await {
        Err(ClientBuildError::AutoDiscovery(_)) => {
            let url = format!("https://{}/", account.user_id.server_name().as_str());
            let url = Url::parse(&url).unwrap();
            create_client_inner(&Some(url), settings).await
        },
        res => res,
    };

    res.expect("Failed to instantiate client")
}

/// Create a new CyrumClient wrapping a matrix-sdk Client
pub fn create_cyrum_client(client: Client) -> CyrumClient {
    CyrumClient::from_client(client)
}

#[derive(Clone)]
pub struct Requester {
    pub client: CyrumClient,
    pub tx: UnboundedSender<WorkerTask>,
}

impl Requester {
    pub fn init(&self, store: AsyncProgramStore) {
        let (reply, response) = oneshot();

        self.tx.send(WorkerTask::Init(store, reply)).unwrap();

        return response.recv();
    }

    pub fn login(&self, style: LoginStyle) -> IambResult<EditInfo> {
        let (reply, response) = oneshot();

        self.tx.send(WorkerTask::Login(style, reply)).unwrap();

        return response.recv();
    }

    pub fn logout(&self, user_id: String) -> IambResult<EditInfo> {
        let (reply, response) = oneshot();

        self.tx.send(WorkerTask::Logout(user_id, reply)).unwrap();

        return response.recv();
    }

    pub fn get_inviter(&self, invite: MatrixRoom) -> IambResult<Option<RoomMember>> {
        let (reply, response) = oneshot();

        self.tx.send(WorkerTask::GetInviter(invite, reply)).unwrap();

        return response.recv();
    }

    pub fn get_room(&self, room_id: OwnedRoomId) -> IambResult<FetchedRoom> {
        let (reply, response) = oneshot();

        self.tx.send(WorkerTask::GetRoom(room_id, reply)).unwrap();

        return response.recv();
    }

    pub fn join_room(&self, name: String) -> IambResult<OwnedRoomId> {
        let (reply, response) = oneshot();

        self.tx.send(WorkerTask::JoinRoom(name, reply)).unwrap();

        return response.recv();
    }

    pub fn members(&self, room_id: OwnedRoomId) -> IambResult<Vec<RoomMember>> {
        let (reply, response) = oneshot();

        self.tx.send(WorkerTask::Members(room_id, reply)).unwrap();

        return response.recv();
    }

    pub fn space_members(&self, space: OwnedRoomId) -> IambResult<Vec<OwnedRoomId>> {
        let (reply, response) = oneshot();

        self.tx.send(WorkerTask::SpaceMembers(space, reply)).unwrap();

        return response.recv();
    }

    pub fn typing_notice(&self, room_id: OwnedRoomId) {
        self.tx.send(WorkerTask::TypingNotice(room_id)).unwrap();
    }

    pub fn verify(&self, act: VerifyAction, sas: SasVerification) -> IambResult<EditInfo> {
        let (reply, response) = oneshot();

        self.tx.send(WorkerTask::Verify(act, sas, reply)).unwrap();

        return response.recv();
    }

    pub fn verify_request(&self, user_id: OwnedUserId) -> IambResult<EditInfo> {
        let (reply, response) = oneshot();

        self.tx.send(WorkerTask::VerifyRequest(user_id, reply)).unwrap();

        return response.recv();
    }
}

pub struct ClientWorker {
    initialized: bool,
    settings: ApplicationSettings,
    client: CyrumClient,
    encryption: CyrumEncryption,
    sync: CyrumSync,
    load_handle: Option<JoinHandle<()>>,
    sync_handle: Option<JoinHandle<()>>,
}

impl ClientWorker {
    pub async fn spawn(client: Client, settings: ApplicationSettings) -> Requester {
        let (tx, rx) = unbounded_channel();
        
        // Create CyrumClient and related components
        let cyrum_client = create_cyrum_client(client.clone());
        let cyrum_encryption = cyrum_client.encryption();
        let cyrum_sync = cyrum_client.sync();

        let mut worker = ClientWorker {
            initialized: false,
            settings,
            client: cyrum_client.clone(),
            encryption: cyrum_encryption,
            sync: cyrum_sync,
            load_handle: None,
            sync_handle: None,
        };

        tokio::spawn(async move {
            worker.work(rx).await;
        });

        return Requester { client: cyrum_client, tx };
    }

    async fn work(&mut self, mut rx: UnboundedReceiver<WorkerTask>) {
        loop {
            let t = rx.recv().await;

            match t {
                Some(task) => self.run(task).await,
                None => {
                    break;
                },
            }
        }

        if let Some(handle) = self.sync_handle.take() {
            handle.abort();
        }
    }

    async fn run(&mut self, task: WorkerTask) {
        match task {
            WorkerTask::Init(store, reply) => {
                assert_eq!(self.initialized, false);
                self.init(store).await;
                reply.send(());
            },
            WorkerTask::JoinRoom(room_id, reply) => {
                assert!(self.initialized);
                reply.send(self.join_room(room_id).await);
            },
            WorkerTask::GetInviter(invited, reply) => {
                assert!(self.initialized);
                reply.send(self.get_inviter(invited).await);
            },
            WorkerTask::GetRoom(room_id, reply) => {
                assert!(self.initialized);
                reply.send(self.get_room(room_id).await);
            },
            WorkerTask::Login(style, reply) => {
                assert!(self.initialized);
                reply.send(self.login_and_sync(style).await);
            },
            WorkerTask::Logout(user_id, reply) => {
                assert!(self.initialized);
                reply.send(self.logout(user_id).await);
            },
            WorkerTask::Members(room_id, reply) => {
                assert!(self.initialized);
                reply.send(self.members(room_id).await);
            },
            WorkerTask::SpaceMembers(space, reply) => {
                assert!(self.initialized);
                reply.send(self.space_members(space).await);
            },
            WorkerTask::TypingNotice(room_id) => {
                assert!(self.initialized);
                self.typing_notice(room_id).await;
            },
            WorkerTask::Verify(act, sas, reply) => {
                assert!(self.initialized);
                reply.send(self.verify(act, sas).await);
            },
            WorkerTask::VerifyRequest(user_id, reply) => {
                assert!(self.initialized);
                reply.send(self.verify_request(user_id).await);
            },
        }
    }

    async fn init(&mut self, store: AsyncProgramStore) {
        // We need to subscribe to events using CyrumSync's streaming interfaces
        let sync = self.sync.clone();
        let store_clone = store.clone();
        
        // Set up task to handle typing notifications
        tokio::spawn(async move {
            let mut typing_stream = sync.subscribe_to_typing().await
                .expect("Failed to subscribe to typing events");
                
            while let Some(Ok((room_id, ev))) = typing_stream.next().await {
                let mut locked = store_clone.lock().await;
                let users = ev.content.user_ids
                    .into_iter()
                    .filter(|u| u != &locked.application.settings.profile.user_id)
                    .collect();
                    
                locked.application.get_room_info(room_id).set_typing(users);
            }
        });
        
        // Set up task to handle presence events
        let store_clone = store.clone();
        let sync = self.sync.clone();
        tokio::spawn(async move {
            let mut presence_stream = sync.subscribe_to_presence().await
                .expect("Failed to subscribe to presence events");
                
            while let Some(Ok(ev)) = presence_stream.next().await {
                let mut locked = store_clone.lock().await;
                locked.application.presences.insert(ev.sender, ev.content.presence);
            }
        });
        
        // Set up task to handle room messages
        let store_clone = store.clone();
        let sync = self.sync.clone();
        let client = self.client.clone();
        let encryption = self.encryption.clone();
        tokio::spawn(async move {
            let mut message_stream = sync.subscribe_to_messages().await
                .expect("Failed to subscribe to message events");
                
            while let Some(Ok((room_id, ev))) = message_stream.next().await {
                // Handle verification requests
                if let Some(msg) = ev.content.msgtype.as_verification_request() {
                    if let Some(room) = client.get_room(&room_id) {
                        if let Some(request) = encryption.verify_user(ev.sender.as_ref()).await.ok() {
                            let _ = request.accept_sas().await;
                        }
                    }
                }
                
                // Update the room info
                let mut locked = store_clone.lock().await;
                let sender = ev.sender.clone();
                let _ = locked.application.presences.get_or_default(sender);
                
                let ChatStore { rooms, picker, settings, .. } = &mut locked.application;
                let info = rooms.get_or_default(room_id.clone());
                
                // For receipt handling with CyrumRoom
                if let Some(room) = client.get_room(&room_id) {
                    drop(locked); // Avoid holding the lock during await
                    
                    // Fetch receipts for this event
                    let event_id = ev.event_id.clone();
                    match room.inner().load_event_receipts(
                        ReceiptType::Read, 
                        ReceiptThread::Main, 
                        &event_id,
                    ).await {
                        Ok(receipts) => {
                            let mut locked = store_clone.lock().await;
                            let info = locked.application.rooms.get_or_default(room_id.clone());
                            for (user_id, _) in receipts {
                                info.set_receipt(user_id, event_id.clone());
                            }
                        },
                        Err(e) => {
                            tracing::warn!(?event_id, "failed to get event receipts: {e}");
                        }
                    }
                    
                    // Re-acquire the lock for the message preview
                    let mut locked = store_clone.lock().await;
                    let ChatStore { rooms, picker, settings, .. } = &mut locked.application;
                    let info = rooms.get_or_default(room_id.clone());
                    
                    // Create the full event
                    let full_ev = SyncMessageLikeEvent::Original(ev.clone())
                        .into_full_event(room_id.clone());
                        
                    // Insert with preview
                    info.insert_with_preview(
                        room_id.clone(),
                        store_clone.clone(),
                        *picker,
                        full_ev,
                        settings,
                        client.inner().media(),
                    );
                }
            }
        });
        
        // Set up tasks for other event types
        // For reactions
        let store_clone = store.clone();
        let client = self.client.clone();
        tokio::spawn(async move {
            // The client.inner() is needed because CyrumSync doesn't yet have a 
            // specific method for reactions - in a full implementation this would
            // be provided by CyrumSync
            let inner_client = client.inner().clone();
            let (sender, mut receiver) = tokio::sync::mpsc::channel(100);
            
            inner_client.add_event_handler(move |ev: SyncMessageLikeEvent<ReactionEventContent>, room: MatrixRoom| {
                let sender = sender.clone();
                let room_id = room.room_id().to_owned();
                
                async move {
                    let _ = sender.send((room_id, ev, room)).await;
                }
            });
            
            while let Some((room_id, ev, room)) = receiver.recv().await {
                let mut locked = store_clone.lock().await;
                let sender = ev.sender().to_owned();
                let _ = locked.application.presences.get_or_default(sender);
                
                let info = locked.application.get_room_info(room_id.clone());
                
                // Handle receipts
                let event_id = ev.event_id();
                drop(locked); // Release lock during await
                
                let receipts = match room.load_event_receipts(
                    ReceiptType::Read, 
                    ReceiptThread::Main, 
                    event_id,
                ).await {
                    Ok(receipts) => receipts,
                    Err(e) => {
                        tracing::warn!(?event_id, "failed to get event receipts: {e}");
                        continue;
                    }
                };
                
                let mut locked = store_clone.lock().await;
                let info = locked.application.get_room_info(room_id.clone());
                
                for (user_id, _) in receipts {
                    info.set_receipt(user_id, event_id.to_owned());
                }
                
                // Insert the reaction
                info.insert_reaction(ev.into_full_event(room_id));
            }
        });
        
        // Set up the background tasks for older messages and receipts
        self.load_handle = tokio::spawn({
            let client = self.client.clone();
            let settings = self.settings.clone();

            async move {
                // Wait for login completion
                while !client.is_logged_in() {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }

                let load = load_older_forever(&client, &store);
                let rcpt = send_receipts_forever(&client, &store);
                let room = refresh_rooms_forever(&client, &store);
                let notifications = register_notifications(client.inner(), &settings, &store);
                let ((), (), (), ()) = tokio::join!(load, rcpt, room, notifications);
            }
        })
        .into();

        self.initialized = true;
    }

    async fn login_and_sync(&mut self, style: LoginStyle) -> IambResult<EditInfo> {
        match style {
            LoginStyle::SessionRestore(session) => {
                // Use the Matrix SDK client directly for session restoration
                // since CyrumClient doesn't expose this method directly
                self.client.inner().restore_session(session).await
                    .map_err(IambError::from)?;
            },
            LoginStyle::Password(password) => {
                // Use CyrumClient's login method
                let username = self.settings.profile.user_id.to_string();
                self.client.login(&username, &password).await
                    .map_err(|e| IambError::MatrixSdk(e.to_string()))?;
                
                // Save the session
                let session = self.client.inner().session().expect("Session should exist after login");
                self.settings.write_session(session)?;
            },
            LoginStyle::SingleSignOn => {
                // For SSO login, we need to use the inner client directly
                // This should eventually be added to CyrumClient
                let resp = self.client.inner()
                    .authentication()
                    .matrix_auth()
                    .login_sso(|url| {
                        let opened = format!(
                            "The following URL should have been opened in your browser:\n    {url}"
                        );

                        async move {
                            tokio::task::spawn_blocking(move || open::that(url));
                            println!("\n{opened}\n");
                            Ok(())
                        }
                    })
                    .initial_device_display_name(initial_devname().as_str())
                    .send()
                    .await
                    .map_err(IambError::from)?;

                let session = MatrixSession::from(&resp);
                self.settings.write_session(session)?;
            },
        }

        // Use CyrumSync to start syncing
        self.sync_handle = tokio::spawn(async move {
            // Use the sync manager from CyrumSync
            let sync = self.sync.clone();
            sync.start_sync().await.expect("Failed to start sync");
        })
        .into();

        Ok(Some(InfoMessage::from("* Successfully logged in!")))
    }

    async fn logout(&mut self, user_id: String) -> IambResult<EditInfo> {
        // Verify that the user is logging out of the correct profile.
        let curr = self.settings.profile.user_id.as_str();

        if user_id != curr {
            let msg = format!("Incorrect user ID (currently logged in as {curr})");
            let err = UIError::Failure(msg);

            return Err(err);
        }

        // Send the logout request using CyrumClient.
        match self.client.logout().await {
            Ok(_) => {
                // Remove the session.json file.
                std::fs::remove_file(&self.settings.session_json)?;
                Ok(Some(InfoMessage::from("Successfully logged out")))
            },
            Err(e) => {
                let msg = format!("Failed to logout: {}", e);
                Err(UIError::Failure(msg))
            }
        }
    }

    async fn direct_message(&mut self, user: OwnedUserId) -> IambResult<OwnedRoomId> {
        // Check for existing DMs first
        for room in self.client.joined_rooms() {
            // Check if it's a direct room
            if !room.is_direct() {
                continue;
            }

            // Check if the user is a member of this room
            let member_fut = room.members();
            match member_fut.await {
                Ok(members) => {
                    if members.iter().any(|m| m.user_id() == &user) {
                        return Ok(room.room_id().to_owned());
                    }
                }
                Err(e) => {
                    warn!("Failed to get room members: {}", e);
                }
            }
        }

        // Create a new DM room using CyrumClient
        match self.client.create_dm_room(user.as_ref()).await {
            Ok(room) => Ok(room.room_id().to_owned()),
            Err(e) => {
                error!(
                    user_id = user.as_str(),
                    err = e.to_string(),
                    "Failed to create direct message room"
                );

                let msg = format!("Could not open a room with {user}");
                Err(UIError::Failure(msg))
            }
        }
    }

    async fn get_inviter(&mut self, invited: MatrixRoom) -> IambResult<Option<RoomMember>> {
        let details = invited.invite_details().await.map_err(IambError::from)?;

        Ok(details.inviter)
    }

    async fn get_room(&mut self, room_id: OwnedRoomId) -> IambResult<FetchedRoom> {
        if let Some(cyrum_room) = self.client.get_room(room_id.as_ref()) {
            // Get the name (display name) of the room
            let name_option = cyrum_room.name();
            let room_name = match name_option {
                Some(name) => RoomDisplayName::Named(name),
                None => RoomDisplayName::Empty,
            };
            
            // For tags, we need to use the inner Matrix Room for now
            // since CyrumRoom doesn't have a tags method yet
            let tags = cyrum_room.inner().tags().await.map_err(IambError::from)?;
            
            // Return the original MatrixRoom for compatibility with existing code
            let matrix_room = cyrum_room.inner().clone();
            
            Ok((matrix_room, room_name, tags))
        } else {
            Err(IambError::UnknownRoom(room_id).into())
        }
    }

    async fn join_room(&mut self, name: String) -> IambResult<OwnedRoomId> {
        if let Ok(room_id) = RoomId::parse(&name) {
            // Join by room ID
            match self.client.join_room_by_id(room_id).await {
                Ok(room) => Ok(room.room_id().to_owned()),
                Err(e) => {
                    let msg = format!("Failed to join room: {}", e);
                    Err(UIError::Failure(msg))
                }
            }
        } else if name.starts_with('#') { 
            // Join by room alias
            match self.client.join_room_by_alias(&name).await {
                Ok(room) => Ok(room.room_id().to_owned()),
                Err(e) => {
                    let msg = format!("Failed to join room: {}", e);
                    Err(UIError::Failure(msg))
                }
            }
        } else if let Ok(user) = OwnedUserId::try_from(name.as_str()) {
            // Create or join a direct message room
            self.direct_message(user).await
        } else {
            let msg = format!("{:?} is not a valid room or user name", name.as_str());
            Err(UIError::Failure(msg))
        }
    }

    async fn members(&mut self, room_id: OwnedRoomId) -> IambResult<Vec<RoomMember>> {
        if let Some(room) = self.client.get_room(room_id.as_ref()) {
            // Use CyrumRoom's members method which returns a MatrixFuture
            match room.members().await {
                Ok(cyrum_members) => {
                    // Convert CyrumRoomMember to RoomMember for compatibility
                    // This is a workaround until we can fully migrate all code to CyrumRoomMember
                    let matrix_members: Vec<RoomMember> = cyrum_members.into_iter()
                        .map(|m| m.inner().clone())
                        .collect();
                    
                    Ok(matrix_members)
                },
                Err(e) => {
                    let msg = format!("Failed to get room members: {}", e);
                    Err(UIError::Failure(msg).into())
                }
            }
        } else {
            Err(IambError::UnknownRoom(room_id).into())
        }
    }

    async fn space_members(&mut self, space: OwnedRoomId) -> IambResult<Vec<OwnedRoomId>> {
        let mut req = SpaceHierarchyRequest::new(space);
        req.limit = Some(1000u32.into());
        req.max_depth = Some(1u32.into());

        let resp = self.client.send(req, None).await.map_err(IambError::from)?;

        let rooms = resp.rooms.into_iter().map(|chunk| chunk.room_id).collect();

        Ok(rooms)
    }

    async fn typing_notice(&mut self, room_id: OwnedRoomId) {
        if let Some(room) = self.client.get_room(room_id.as_ref()) {
            // Use CyrumRoom's typing_notice method
            let _ = room.typing_notice(true).await;
        }
    }

    async fn verify(&self, action: VerifyAction, sas: SasVerification) -> IambResult<EditInfo> {
        // For this method, we need to continue using the native SasVerification directly
        // since the CyrumEncryption wrapper doesn't currently have a way to wrap 
        // an existing SasVerification object.
        
        // In the future, we should enhance CyrumEncryption to handle this case.
        match action {
            VerifyAction::Accept => {
                sas.accept().await.map_err(IambError::from)?;
                Ok(Some(InfoMessage::from("Accepted verification request")))
            },
            VerifyAction::Confirm => {
                if sas.is_done() || sas.is_cancelled() {
                    let msg = "Can only confirm in-progress verifications!";
                    let err = UIError::Failure(msg.into());
                    return Err(err);
                }

                sas.confirm().await.map_err(IambError::from)?;
                Ok(Some(InfoMessage::from("Confirmed verification")))
            },
            VerifyAction::Cancel => {
                if sas.is_done() || sas.is_cancelled() {
                    let msg = "Can only cancel in-progress verifications!";
                    let err = UIError::Failure(msg.into());
                    return Err(err);
                }

                sas.cancel().await.map_err(IambError::from)?;
                Ok(Some(InfoMessage::from("Cancelled verification")))
            },
            VerifyAction::Mismatch => {
                if sas.is_done() || sas.is_cancelled() {
                    let msg = "Can only cancel in-progress verifications!";
                    let err = UIError::Failure(msg.into());
                    return Err(err);
                }

                sas.mismatch().await.map_err(IambError::from)?;
                Ok(Some(InfoMessage::from("Cancelled verification")))
            },
        }
    }

    async fn verify_request(&self, user_id: OwnedUserId) -> IambResult<EditInfo> {
        // Use CyrumEncryption to verify the user
        match self.encryption.verify_user(user_id.as_ref()).await {
            Ok(verification_request) => {
                // The verification request was successfully created
                let info = format!("Sent verification request to {user_id}");
                Ok(Some(InfoMessage::from(info)))
            },
            Err(e) => {
                let msg = format!("Could not verify user {user_id}: {}", e);
                Err(UIError::Failure(msg))
            }
        }
    }
}
