//! # Windows for the User Interface
//!
//! This module contains the logic for rendering windows, and handling UI actions that get
//! delegated to individual windows/UI elements (e.g., typing text or selecting a list item).
//!
//! The window system is built around the `MatrixWindow` trait which provides a unified interface
//! for all window types, allowing them to be managed by the window registry and rendered in tabs.

use std::cmp::{Ord, Ordering, PartialOrd};
use std::fmt::{self, Display};
use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, Instant};

use matrix_sdk::{
    encryption::verification::{format_emojis, SasVerification},
    room::{Room as MatrixRoom, RoomMember},
    ruma::{
        events::room::member::MembershipState,
        events::tag::{TagName, Tags},
        OwnedRoomAliasId,
        OwnedRoomId,
        RoomAliasId,
        RoomId,
    },
};

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::StatefulWidget,
};

use crate::base::{
    ChatStore, 
    maxtryxBufferId, 
    maxtryxError, 
    maxtryxId, 
    maxtryxInfo, 
    maxtryxResult, 
    Need,
    ProgramContext, 
    ProgramStore, 
    SortColumn, 
    SortFieldRoom, 
    SortFieldUser, 
    SortOrder, 
    UnreadInfo,
};

use crate::modal::{
    Action, ActionResult, MatrixAction, DialogAction, EditAction, EditorAction, MovementAction,
    PositionList, MoveDir1D, ScrollStyle, WindowAction,
};

use self::{room::RoomState, welcome::WelcomeState};
use crate::message::MessageTimeStamp;

pub mod room;
pub mod welcome;

type MatrixRoomInfo = Arc<(MatrixRoom, Option<Tags>)>;

const MEMBER_FETCH_DEBOUNCE: Duration = Duration::from_secs(5);

#[inline]
fn bold_style() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

#[inline]
fn bold_span(s: &str) -> Span {
    Span::styled(s, bold_style())
}

#[inline]
fn bold_spans(s: &str) -> Line {
    bold_span(s).into()
}

#[inline]
fn selected_style(selected: bool) -> Style {
    if selected {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    }
}

#[inline]
fn selected_span(s: &str, selected: bool) -> Span {
    Span::styled(s, selected_style(selected))
}

#[inline]
fn selected_text(s: &str, selected: bool) -> Text {
    Text::from(selected_span(s, selected))
}

fn name_and_labels(name: &str, unread: bool, style: Style) -> (Span<'_>, Vec<Vec<Span<'_>>>) {
    let name_style = if unread {
        style.add_modifier(Modifier::BOLD)
    } else {
        style
    };

    let name = Span::styled(name, name_style);
    let labels = if unread {
        vec![vec![Span::styled("Unread", style)]]
    } else {
        vec![]
    };

    (name, labels)
}

/// Sort `Some` to be less than `None` so that list items with values come before those without.
#[inline]
fn some_cmp<T, F>(a: Option<T>, b: Option<T>, f: F) -> Ordering
where
    F: Fn(&T, &T) -> Ordering,
{
    match (a, b) {
        (Some(a), Some(b)) => f(&a, &b),
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
    }
}

/// Trait for something that can be rendered as a room-like item
trait RoomLikeItem {
    fn room_id(&self) -> &RoomId;
    fn has_tag(&self, tag: TagName) -> bool;
    fn is_unread(&self) -> bool;
    fn recent_ts(&self) -> Option<&MessageTimeStamp>;
    fn alias(&self) -> Option<&RoomAliasId>;
    fn name(&self) -> &str;
}

fn room_cmp<T: RoomLikeItem>(a: &T, b: &T, field: &SortFieldRoom) -> Ordering {
    match field {
        SortFieldRoom::Favorite => {
            let fava = a.has_tag(TagName::Favorite);
            let favb = b.has_tag(TagName::Favorite);

            // If a has Favorite and b doesn't, it should sort earlier in room list.
            favb.cmp(&fava)
        },
        SortFieldRoom::LowPriority => {
            let lowa = a.has_tag(TagName::LowPriority);
            let lowb = b.has_tag(TagName::LowPriority);

            // If a has LowPriority and b doesn't, it should sort later in room list.
            lowa.cmp(&lowb)
        },
        SortFieldRoom::Name => a.name().cmp(b.name()),
        SortFieldRoom::Alias => some_cmp(a.alias(), b.alias(), Ord::cmp),
        SortFieldRoom::RoomId => a.room_id().cmp(b.room_id()),
        SortFieldRoom::Unread => {
            // Sort true (unread) before false (read)
            b.is_unread().cmp(&a.is_unread())
        },
        SortFieldRoom::Recent => {
            // sort larger timestamps towards the top.
            some_cmp(a.recent_ts(), b.recent_ts(), |a, b| b.cmp(a))
        },
    }
}

/// Compare two rooms according the configured sort criteria.
fn room_fields_cmp<T: RoomLikeItem>(
    a: &T,
    b: &T,
    fields: &[SortColumn<SortFieldRoom>],
) -> Ordering {
    for SortColumn(field, order) in fields {
        match (room_cmp(a, b, field), order) {
            (Ordering::Equal, _) => continue,
            (o, SortOrder::Ascending) => return o,
            (o, SortOrder::Descending) => return o.reverse(),
        }
    }

    // Break ties on ascending room id.
    room_cmp(a, b, &SortFieldRoom::RoomId)
}

fn user_cmp(a: &MemberItem, b: &MemberItem, field: &SortFieldUser) -> Ordering {
    let a_id = a.member.user_id();
    let b_id = b.member.user_id();

    match field {
        SortFieldUser::UserId => a_id.cmp(b_id),
        SortFieldUser::LocalPart => a_id.localpart().cmp(b_id.localpart()),
        SortFieldUser::Server => a_id.server_name().cmp(b_id.server_name()),
        SortFieldUser::PowerLevel => {
            // Sort higher power levels towards the top of the list.
            b.member.power_level().cmp(&a.member.power_level())
        },
    }
}

fn user_fields_cmp(
    a: &MemberItem,
    b: &MemberItem,
    fields: &[SortColumn<SortFieldUser>],
) -> Ordering {
    for SortColumn(field, order) in fields {
        match (user_cmp(a, b, field), order) {
            (Ordering::Equal, _) => continue,
            (o, SortOrder::Ascending) => return o,
            (o, SortOrder::Descending) => return o.reverse(),
        }
    }

    // Break ties on ascending user id.
    user_cmp(a, b, &SortFieldUser::UserId)
}

fn tag_to_span(tag: &TagName, style: Style) -> Vec<Span<'_>> {
    match tag {
        TagName::Favorite => vec![Span::styled("Favorite", style)],
        TagName::LowPriority => vec![Span::styled("Low Priority", style)],
        TagName::ServerNotice => vec![Span::styled("Server Notice", style)],
        TagName::User(tag) => {
            vec![
                Span::styled("User Tag: ", style),
                Span::styled(tag.as_ref(), style),
            ]
        },
        tag => vec![Span::styled(format!("{tag:?}"), style)],
    }
}

fn append_tags<'a>(tags: Vec<Vec<Span<'a>>>, spans: &mut Vec<Span<'a>>, style: Style) {
    if tags.is_empty() {
        return;
    }

    spans.push(Span::styled(" (", style));

    for (i, tag) in tags.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(", ", style));
        }

        spans.extend(tag);
    }

    spans.push(Span::styled(")", style));
}

/// The main window enum that contains all window types
#[derive(Debug)]
pub enum MatrixWindow {
    /// A welcome window shown on startup
    Welcome(WelcomeState),
    /// A room window for chat
    Room(RoomState),
    /// Room list window
    RoomList(RoomListState),
    /// Direct messages list window
    DirectList(DirectListState),
    /// Space list window
    SpaceList(SpaceListState),
    /// Member list window
    MemberList(MemberListState, OwnedRoomId, Option<Instant>),
    /// Verification list window
    VerifyList(VerifyListState),
    /// Combined chats list window
    ChatList(ChatListState),
    /// Unread messages list window
    UnreadList(UnreadListState),
}

impl MatrixWindow {
    /// Create a new window
    pub fn new(id: maxtryxId, store: &mut ProgramStore) -> maxtryxResult<Self> {
        match id {
            maxtryxId::Room(room_id, thread) => {
                let (room, name, tags) = store.application.worker.get_room(room_id)?;
                let room = RoomState::new(room, thread, name, tags, store);

                store.application.need_load.insert(room.id().to_owned(), Need::MEMBERS);
                Ok(MatrixWindow::Room(room))
            },
            maxtryxId::DirectList => {
                let list = DirectListState::new(maxtryxBufferId::DirectList, vec![]);
                Ok(MatrixWindow::DirectList(list))
            },
            maxtryxId::MemberList(room_id) => {
                let id = maxtryxBufferId::MemberList(room_id.clone());
                let list = MemberListState::new(id, vec![]);
                Ok(MatrixWindow::MemberList(list, room_id, None))
            },
            maxtryxId::RoomList => {
                let list = RoomListState::new(maxtryxBufferId::RoomList, vec![]);
                Ok(MatrixWindow::RoomList(list))
            },
            maxtryxId::SpaceList => {
                let list = SpaceListState::new(maxtryxBufferId::SpaceList, vec![]);
                Ok(MatrixWindow::SpaceList(list))
            },
            maxtryxId::VerifyList => {
                let list = VerifyListState::new(maxtryxBufferId::VerifyList, vec![]);
                Ok(MatrixWindow::VerifyList(list))
            },
            maxtryxId::Welcome => {
                let win = WelcomeState::new(store);
                Ok(MatrixWindow::Welcome(win))
            },
            maxtryxId::ChatList => {
                let list = ChatListState::new(maxtryxBufferId::ChatList, vec![]);
                Ok(MatrixWindow::ChatList(list))
            },
            maxtryxId::UnreadList => {
                let list = UnreadListState::new(maxtryxBufferId::UnreadList, vec![]);
                Ok(MatrixWindow::UnreadList(list))
            },
        }
    }

    /// Find a window by name
    pub fn find(name: String, store: &mut ProgramStore) -> maxtryxResult<Self> {
        let ChatStore { names, worker, .. } = &mut store.application;

        if let Some(room) = names.get_mut(&name) {
            let id = maxtryxId::Room(room.clone(), None);
            MatrixWindow::new(id, store)
        } else {
            let room_id = worker.join_room(name.clone())?;
            names.insert(name, room_id.clone());

            let (room, name, tags) = store.application.worker.get_room(room_id)?;
            let room = RoomState::new(room, None, name, tags, store);

            store.application.need_load.insert(room.id().to_owned(), Need::MEMBERS);
            Ok(MatrixWindow::Room(room))
        }
    }

    /// Get the window ID
    pub fn id(&self) -> maxtryxId {
        match self {
            MatrixWindow::Room(room) => maxtryxId::Room(room.id().to_owned(), room.thread().cloned()),
            MatrixWindow::DirectList(_) => maxtryxId::DirectList,
            MatrixWindow::MemberList(_, room_id, _) => maxtryxId::MemberList(room_id.clone()),
            MatrixWindow::RoomList(_) => maxtryxId::RoomList,
            MatrixWindow::SpaceList(_) => maxtryxId::SpaceList,
            MatrixWindow::VerifyList(_) => maxtryxId::VerifyList,
            MatrixWindow::Welcome(_) => maxtryxId::Welcome,
            MatrixWindow::ChatList(_) => maxtryxId::ChatList,
            MatrixWindow::UnreadList(_) => maxtryxId::UnreadList,
        }
    }

    /// Get the tab title for this window
    pub fn tab_title(&self, store: &mut ProgramStore) -> Line {
        match self {
            MatrixWindow::DirectList(_) => bold_spans("Direct Messages"),
            MatrixWindow::RoomList(_) => bold_spans("Rooms"),
            MatrixWindow::SpaceList(_) => bold_spans("Spaces"),
            MatrixWindow::VerifyList(_) => bold_spans("Verifications"),
            MatrixWindow::Welcome(_) => bold_spans("Welcome to Matrix"),
            MatrixWindow::ChatList(_) => bold_spans("DMs & Rooms"),
            MatrixWindow::UnreadList(_) => bold_spans("Unread Messages"),

            MatrixWindow::Room(w) => {
                let title = store.application.get_room_title(w.id());
                Line::from(title)
            },
            MatrixWindow::MemberList(state, room_id, _) => {
                let title = store.application.get_room_title(room_id.as_ref());
                let n = state.len();
                let v = vec![
                    bold_span("Room Members "),
                    Span::styled(format!("({n}): "), bold_style()),
                    title.into(),
                ];
                Line::from(v)
            },
        }
    }

    /// Get the window title for this window
    pub fn window_title(&self, store: &mut ProgramStore) -> Line {
        match self {
            MatrixWindow::DirectList(_) => bold_spans("Direct Messages"),
            MatrixWindow::RoomList(_) => bold_spans("Rooms"),
            MatrixWindow::SpaceList(_) => bold_spans("Spaces"),
            MatrixWindow::VerifyList(_) => bold_spans("Verifications"),
            MatrixWindow::Welcome(_) => bold_spans("Welcome to Matrix"),
            MatrixWindow::ChatList(_) => bold_spans("DMs & Rooms"),
            MatrixWindow::UnreadList(_) => bold_spans("Unread Messages"),

            MatrixWindow::Room(w) => w.get_title(store),
            MatrixWindow::MemberList(state, room_id, _) => {
                let title = store.application.get_room_title(room_id.as_ref());
                let n = state.len();
                let v = vec![
                    bold_span("Room Members "),
                    Span::styled(format!("({n}): "), bold_style()),
                    title.into(),
                ];
                Line::from(v)
            },
        }
    }

    /// Draw the window
    pub fn draw(&mut self, area: Rect, buf: &mut Buffer, focused: bool, store: &mut ProgramStore) {
        match self {
            MatrixWindow::Room(state) => state.draw(area, buf, focused, store),
            MatrixWindow::DirectList(state) => {
                self.draw_list(state, area, buf, focused, store, "Direct Messages", 
                    "No direct messages yet!");
            },
            MatrixWindow::MemberList(state, room_id, last_fetch) => {
                // Check if we need to fetch members
                let need_fetch = match last_fetch {
                    Some(i) => i.elapsed() >= MEMBER_FETCH_DEBOUNCE,
                    None => true,
                };

                if need_fetch {
                    if let Ok(mems) = store.application.worker.members(room_id.clone()) {
                        let mut items = mems
                            .into_iter()
                            .map(|m| MemberItem::new(m, room_id.clone()))
                            .collect::<Vec<_>>();
                        let fields = &store.application.settings.tunables.sort.members;
                        items.sort_by(|a, b| user_fields_cmp(a, b, fields));
                        state.set(items);
                        *last_fetch = Some(Instant::now());
                    }
                }

                // Create title with member count
                let title = format!("Room Members ({})", state.len());
                self.draw_list(state, area, buf, focused, store, &title, "No users here yet!");
            },
            MatrixWindow::RoomList(state) => {
                // Populate room list from store
                let mut items = store
                    .application
                    .sync_info
                    .rooms
                    .clone()
                    .into_iter()
                    .map(|room_info| RoomItem::new(room_info, store))
                    .collect::<Vec<_>>();
                let fields = &store.application.settings.tunables.sort.rooms;
                items.sort_by(|a, b| room_fields_cmp(a, b, fields));
                state.set(items);

                self.draw_list(state, area, buf, focused, store, "Rooms", 
                    "You haven't joined any rooms yet");
            },
            MatrixWindow::SpaceList(state) => {
                // Populate space list from store
                let mut items = store
                    .application
                    .sync_info
                    .spaces
                    .clone()
                    .into_iter()
                    .map(|room| SpaceItem::new(room, store))
                    .collect::<Vec<_>>();
                let fields = &store.application.settings.tunables.sort.spaces;
                items.sort_by(|a, b| room_fields_cmp(a, b, fields));
                state.set(items);

                self.draw_list(state, area, buf, focused, store, "Spaces", 
                    "You haven't joined any spaces yet");
            },
            MatrixWindow::VerifyList(state) => {
                // Populate verification list from store
                let verifications = &store.application.verifications;
                let mut items = verifications
                    .iter()
                    .map(VerifyItem::from)
                    .collect::<Vec<_>>();

                // Sort the active verifications towards the top
                items.sort();
                state.set(items);

                self.draw_list(state, area, buf, focused, store, "Verifications", 
                    "No in-progress verifications");
            },
            MatrixWindow::Welcome(state) => {
                state.draw(area, buf, focused, store);
            },
            MatrixWindow::ChatList(state) => {
                // Populate chat list (combined rooms and DMs) from store
                let mut items = store
                    .application
                    .sync_info
                    .rooms
                    .clone()
                    .into_iter()
                    .map(|room_info| GenericChatItem::new(room_info, store, false))
                    .collect::<Vec<_>>();

                let dms = store
                    .application
                    .sync_info
                    .dms
                    .clone()
                    .into_iter()
                    .map(|room_info| GenericChatItem::new(room_info, store, true));

                items.extend(dms);

                let fields = &store.application.settings.tunables.sort.chats;
                items.sort_by(|a, b| room_fields_cmp(a, b, fields));
                state.set(items);

                self.draw_list(state, area, buf, focused, store, "DMs & Rooms", 
                    "You do not have rooms or dms yet");
            },
            MatrixWindow::UnreadList(state) => {
                // Populate unread list from store (filter by unread status)
                let mut items = store
                    .application
                    .sync_info
                    .rooms
                    .clone()
                    .into_iter()
                    .map(|room_info| GenericChatItem::new(room_info, store, false))
                    .filter(RoomLikeItem::is_unread)
                    .collect::<Vec<_>>();

                let dms = store
                    .application
                    .sync_info
                    .dms
                    .clone()
                    .into_iter()
                    .map(|room_info| GenericChatItem::new(room_info, store, true))
                    .filter(RoomLikeItem::is_unread);

                items.extend(dms);

                let fields = &store.application.settings.tunables.sort.chats;
                items.sort_by(|a, b| room_fields_cmp(a, b, fields));
                state.set(items);

                self.draw_list(state, area, buf, focused, store, "Unread Messages", 
                    "No unread messages");
            },
        }
    }
    
    /// Draw a list window with the given state and title
    fn draw_list<T: Clone + Display>(&self, 
        state: &mut ListState<T>, 
        area: Rect, 
        buf: &mut Buffer, 
        focused: bool, 
        _store: &mut ProgramStore,
        title: &str,
        empty_message: &str
    ) {
        // Create a block with a border and title
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(if focused {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            });
            
        // Render the block
        let inner_area = block.inner(area);
        block.render(area, buf);
        
        // Update list with viewport height
        state.set_height(inner_area.height as usize);
        
        if state.is_empty() {
            // Render empty message
            let text = Text::from(empty_message);
            let paragraph = Paragraph::new(text)
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Gray));
                
            paragraph.render(inner_area, buf);
            return;
        }
        
        // Calculate how many items we can display
        let height = inner_area.height as usize;
        
        // Get the subset of items to render based on scroll position
        let offset = state.offset();
        let end_idx = (offset + height).min(state.len());
        let visible_items = &state.items()[offset..end_idx];
        
        // Render each visible item
        for (i, item) in visible_items.iter().enumerate() {
            let list_idx = offset + i;
            let is_selected = state.selected() == Some(list_idx);
            
            // Create text representation for the item
            let item_text = item.to_string();
            let style = if is_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            
            // Create a span with the appropriate style
            let span = Span::styled(item_text, style);
            let line = Line::from(span);
            
            // Draw the item
            let y = inner_area.y + i as u16;
            buf.set_line(inner_area.x, y, &line, inner_area.width);
        }
    }

    /// Create a duplicate of this window
    pub fn duplicate(&self, store: &mut ProgramStore) -> Self {
        match self {
            MatrixWindow::Room(w) => MatrixWindow::Room(w.dup(store)),
            MatrixWindow::DirectList(w) => MatrixWindow::DirectList(w.dup(store)),
            MatrixWindow::MemberList(w, room_id, last_fetch) => {
                MatrixWindow::MemberList(w.dup(store), room_id.clone(), *last_fetch)
            },
            MatrixWindow::RoomList(w) => MatrixWindow::RoomList(w.dup(store)),
            MatrixWindow::SpaceList(w) => MatrixWindow::SpaceList(w.dup(store)),
            MatrixWindow::VerifyList(w) => MatrixWindow::VerifyList(w.dup(store)),
            MatrixWindow::Welcome(w) => MatrixWindow::Welcome(w.dup(store)),
            MatrixWindow::ChatList(w) => MatrixWindow::ChatList(w.dup(store)),
            MatrixWindow::UnreadList(w) => MatrixWindow::UnreadList(w.dup(store)),
        }
    }

    /// Close the window
    pub fn close(&mut self, store: &mut ProgramStore) -> bool {
        match self {
            MatrixWindow::Room(w) => w.close(store),
            MatrixWindow::DirectList(w) => true, // Lists can always be closed
            MatrixWindow::MemberList(_, _, _) => true,
            MatrixWindow::RoomList(_) => true,
            MatrixWindow::SpaceList(_) => true,
            MatrixWindow::VerifyList(_) => true,
            MatrixWindow::Welcome(_) => true,
            MatrixWindow::ChatList(_) => true,
            MatrixWindow::UnreadList(_) => true,
        }
    }

    /// Toggle focus between components in a window
    pub fn focus_toggle(&mut self) {
        if let MatrixWindow::Room(w) = self {
            w.focus_toggle()
        }
    }

    /// Execute editor actions on the window
    pub fn execute_action(
        &mut self,
        action: &EditorAction,
        ctx: &ProgramContext,
        store: &mut ProgramStore,
    ) -> ActionResult {
        match self {
            MatrixWindow::Room(w) => w.editor_command(action, ctx, store),
            MatrixWindow::DirectList(w) => w.editor_command(action, ctx, store),
            MatrixWindow::MemberList(w, _, _) => w.editor_command(action, ctx, store),
            MatrixWindow::RoomList(w) => w.editor_command(action, ctx, store),
            MatrixWindow::SpaceList(w) => w.editor_command(action, ctx, store),
            MatrixWindow::VerifyList(w) => w.editor_command(action, ctx, store),
            MatrixWindow::Welcome(w) => w.editor_command(action, ctx, store),
            MatrixWindow::ChatList(w) => w.editor_command(action, ctx, store),
            MatrixWindow::UnreadList(w) => w.editor_command(action, ctx, store),
        }
    }

    /// Jump to a position in the window
    pub fn jump(
        &mut self,
        list: PositionList,
        dir: MoveDir1D,
        count: usize,
        ctx: &ProgramContext,
    ) -> maxtryxResult<usize> {
        match self {
            MatrixWindow::Room(w) => w.jump(list, dir, count, ctx),
            MatrixWindow::DirectList(w) => w.jump(list, dir, count, ctx),
            MatrixWindow::MemberList(w, _, _) => w.jump(list, dir, count, ctx),
            MatrixWindow::RoomList(w) => w.jump(list, dir, count, ctx),
            MatrixWindow::SpaceList(w) => w.jump(list, dir, count, ctx),
            MatrixWindow::VerifyList(w) => w.jump(list, dir, count, ctx),
            MatrixWindow::Welcome(w) => w.jump(list, dir, count, ctx),
            MatrixWindow::ChatList(w) => w.jump(list, dir, count, ctx),
            MatrixWindow::UnreadList(w) => w.jump(list, dir, count, ctx),
        }
    }

    /// Scroll the window content
    pub fn scroll(
        &mut self,
        style: &ScrollStyle,
        ctx: &ProgramContext,
        store: &mut ProgramStore,
    ) -> ActionResult {
        match self {
            MatrixWindow::Room(w) => w.scroll(style, ctx, store),
            MatrixWindow::DirectList(w) => w.scroll(style, ctx, store),
            MatrixWindow::MemberList(w, _, _) => w.scroll(style, ctx, store),
            MatrixWindow::RoomList(w) => w.scroll(style, ctx, store),
            MatrixWindow::SpaceList(w) => w.scroll(style, ctx, store),
            MatrixWindow::VerifyList(w) => w.scroll(style, ctx, store),
            MatrixWindow::Welcome(w) => w.scroll(style, ctx, store),
            MatrixWindow::ChatList(w) => w.scroll(style, ctx, store),
            MatrixWindow::UnreadList(w) => w.scroll(style, ctx, store),
        }
    }

    /// Get the cursor position for the terminal
    pub fn get_cursor_position(&self) -> Option<(u16, u16)> {
        match self {
            MatrixWindow::Room(w) => w.get_term_cursor().map(|tc| (tc.col, tc.row)),
            MatrixWindow::DirectList(w) => w.get_term_cursor().map(|tc| (tc.col, tc.row)),
            MatrixWindow::MemberList(w, _, _) => w.get_term_cursor().map(|tc| (tc.col, tc.row)),
            MatrixWindow::RoomList(w) => w.get_term_cursor().map(|tc| (tc.col, tc.row)),
            MatrixWindow::SpaceList(w) => w.get_term_cursor().map(|tc| (tc.col, tc.row)),
            MatrixWindow::VerifyList(w) => w.get_term_cursor().map(|tc| (tc.col, tc.row)),
            MatrixWindow::Welcome(w) => w.get_term_cursor().map(|tc| (tc.col, tc.row)),
            MatrixWindow::ChatList(w) => w.get_term_cursor().map(|tc| (tc.col, tc.row)),
            MatrixWindow::UnreadList(w) => w.get_term_cursor().map(|tc| (tc.col, tc.row)),
        }
    }

    /// Get completions from the window
    pub fn get_completions(&self) -> Option<Vec<String>> {
        match self {
            MatrixWindow::Room(w) => w.get_completions().map(|cl| cl.items().collect()),
            MatrixWindow::DirectList(w) => w.get_completions().map(|cl| cl.items().collect()),
            MatrixWindow::MemberList(w, _, _) => w.get_completions().map(|cl| cl.items().collect()),
            MatrixWindow::RoomList(w) => w.get_completions().map(|cl| cl.items().collect()),
            MatrixWindow::SpaceList(w) => w.get_completions().map(|cl| cl.items().collect()),
            MatrixWindow::VerifyList(w) => w.get_completions().map(|cl| cl.items().collect()),
            MatrixWindow::Welcome(w) => w.get_completions().map(|cl| cl.items().collect()),
            MatrixWindow::ChatList(w) => w.get_completions().map(|cl| cl.items().collect()),
            MatrixWindow::UnreadList(w) => w.get_completions().map(|cl| cl.items().collect()),
        }
    }

    /// Get the word at the cursor
    pub fn get_cursor_word(&self) -> Option<String> {
        match self {
            MatrixWindow::Room(w) => w.get_cursor_word(&crate::modal::WordStyle::Small),
            MatrixWindow::DirectList(w) => w.get_cursor_word(&crate::modal::WordStyle::Small),
            MatrixWindow::MemberList(w, _, _) => w.get_cursor_word(&crate::modal::WordStyle::Small),
            MatrixWindow::RoomList(w) => w.get_cursor_word(&crate::modal::WordStyle::Small),
            MatrixWindow::SpaceList(w) => w.get_cursor_word(&crate::modal::WordStyle::Small),
            MatrixWindow::VerifyList(w) => w.get_cursor_word(&crate::modal::WordStyle::Small),
            MatrixWindow::Welcome(w) => w.get_cursor_word(&crate::modal::WordStyle::Small),
            MatrixWindow::ChatList(w) => w.get_cursor_word(&crate::modal::WordStyle::Small),
            MatrixWindow::UnreadList(w) => w.get_cursor_word(&crate::modal::WordStyle::Small),
        }
    }

    /// Get selected text
    pub fn get_selected_text(&self) -> Option<String> {
        match self {
            MatrixWindow::Room(w) => w.get_selected_word(),
            MatrixWindow::DirectList(w) => w.get_selected_word(),
            MatrixWindow::MemberList(w, _, _) => w.get_selected_word(),
            MatrixWindow::RoomList(w) => w.get_selected_word(),
            MatrixWindow::SpaceList(w) => w.get_selected_word(),
            MatrixWindow::VerifyList(w) => w.get_selected_word(),
            MatrixWindow::Welcome(w) => w.get_selected_word(),
            MatrixWindow::ChatList(w) => w.get_selected_word(),
            MatrixWindow::UnreadList(w) => w.get_selected_word(),
        }
    }
}

/// Generic list state for all list types
#[derive(Debug, Clone)]
pub struct ListState<T> {
    /// Buffer ID for the list
    id: maxtryxBufferId,
    /// Items in the list
    items: Vec<T>,
    /// Currently selected item index
    selected: Option<usize>,
    /// Offset from the top of the list for scrolling
    offset: usize,
    /// Last known viewport height
    height: usize,
}

impl<T> ListState<T> {
    /// Create a new list state
    pub fn new(id: maxtryxBufferId, items: Vec<T>) -> Self {
        let selected = if items.is_empty() { None } else { Some(0) };
        Self {
            id,
            items,
            selected,
            offset: 0,
            height: 0,
        }
    }

    /// Get the buffer ID
    pub fn id(&self) -> &maxtryxBufferId {
        &self.id
    }
    
    /// Get the items in the list
    pub fn items(&self) -> &[T] {
        &self.items
    }
    
    /// Get a mutable reference to the items
    pub fn items_mut(&mut self) -> &mut Vec<T> {
        &mut self.items
    }
    
    /// Set the items in the list
    pub fn set(&mut self, items: Vec<T>) {
        // Maintain selection if possible
        if let Some(idx) = self.selected {
            self.selected = if items.is_empty() {
                None
            } else if idx < items.len() {
                Some(idx)
            } else {
                Some(items.len() - 1)
            };
        } else if !items.is_empty() {
            self.selected = Some(0);
        }
        
        self.items = items;
        self.update_offset();
    }
    
    /// Get the selected item index
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }
    
    /// Set the selected item index
    pub fn select(&mut self, index: Option<usize>) {
        let max = self.items.len().saturating_sub(1);
        self.selected = match index {
            Some(i) if i <= max => Some(i),
            Some(_) if max > 0 => Some(max),
            _ => None,
        };
        
        self.update_offset();
    }
    
    /// Move the selection up
    pub fn select_prev(&mut self, wrap: bool) {
        if self.items.is_empty() {
            return;
        }
        
        self.selected = match self.selected {
            Some(0) if wrap => Some(self.items.len() - 1),
            Some(0) => Some(0),
            Some(i) => Some(i - 1),
            None => Some(0),
        };
        
        self.update_offset();
    }
    
    /// Move the selection down
    pub fn select_next(&mut self, wrap: bool) {
        if self.items.is_empty() {
            return;
        }
        
        let max = self.items.len() - 1;
        self.selected = match self.selected {
            Some(i) if i >= max && wrap => Some(0),
            Some(i) if i >= max => Some(max),
            Some(i) => Some(i + 1),
            None => Some(0),
        };
        
        self.update_offset();
    }
    
    /// Select the first item
    pub fn select_first(&mut self) {
        if !self.items.is_empty() {
            self.selected = Some(0);
            self.offset = 0;
        }
    }
    
    /// Select the last item
    pub fn select_last(&mut self) {
        if !self.items.is_empty() {
            self.selected = Some(self.items.len() - 1);
            self.update_offset();
        }
    }
    
    /// Update the scroll offset based on the selected item and viewport height
    fn update_offset(&mut self) {
        if let Some(selected) = self.selected {
            if selected < self.offset {
                // Selected item is above the visible area
                self.offset = selected;
            } else if self.height > 0 && selected >= self.offset + self.height {
                // Selected item is below the visible area
                self.offset = selected.saturating_sub(self.height) + 1;
            }
        }
    }
    
    /// Set the viewport height
    pub fn set_height(&mut self, height: usize) {
        self.height = height;
        self.update_offset();
    }
    
    /// Get the current offset
    pub fn offset(&self) -> usize {
        self.offset
    }
    
    /// Set the offset directly
    pub fn set_offset(&mut self, offset: usize) {
        let max_offset = self.items.len().saturating_sub(self.height);
        self.offset = offset.min(max_offset);
    }
    
    /// Scroll the list up
    pub fn scroll_up(&mut self, amount: usize) {
        self.offset = self.offset.saturating_sub(amount);
    }
    
    /// Scroll the list down
    pub fn scroll_down(&mut self, amount: usize) {
        let max_offset = self.items.len().saturating_sub(self.height);
        self.offset = (self.offset + amount).min(max_offset);
    }
    
    /// Scroll to the top of the list
    pub fn scroll_to_top(&mut self) {
        self.offset = 0;
    }
    
    /// Scroll to the bottom of the list
    pub fn scroll_to_bottom(&mut self) {
        if self.height > 0 {
            let max_offset = self.items.len().saturating_sub(self.height);
            self.offset = max_offset;
        }
    }
    
    /// Get the visible items
    pub fn visible_items(&self) -> &[T] {
        let end = (self.offset + self.height).min(self.items.len());
        &self.items[self.offset..end]
    }
    
    /// Get the selected item
    pub fn selected_item(&self) -> Option<&T> {
        self.selected.and_then(|i| self.items.get(i))
    }
    
    /// Get the number of items
    pub fn len(&self) -> usize {
        self.items.len()
    }
    
    /// Check if the list is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    
    /// Clear the list
    pub fn clear(&mut self) {
        self.items.clear();
        self.selected = None;
        self.offset = 0;
    }
}

impl<T: Clone> ListState<T> {
    /// Create a duplicate of this list state
    pub fn dup(&self, _store: &mut ProgramStore) -> Self {
        Self {
            id: self.id.clone(),
            items: self.items.clone(),
            selected: self.selected,
            offset: self.offset,
            height: self.height,
        }
    }
}

/// Defines various item types for list states

/// Item for generic chat lists (combines rooms and DMs)
#[derive(Debug, Clone)]
pub struct GenericChatItem {
    /// Room information
    room_info: MatrixRoomInfo,
    /// Room name
    name: String,
    /// Room alias
    alias: Option<OwnedRoomAliasId>,
    /// Is this a direct message
    is_dm: bool,
    /// Unread message information
    unread: UnreadInfo,
}

impl GenericChatItem {
    /// Create a new generic chat item
    pub fn new(room_info: MatrixRoomInfo, store: &mut ProgramStore, is_dm: bool) -> Self {
        let room = &room_info.deref().0;
        let room_id = room.room_id();

        let info = store.application.rooms.get_or_default(room_id.to_owned());
        let name = info.name.clone().unwrap_or_default();
        let alias = room.canonical_alias();
        let unread = info.unreads(&store.application.settings);
        info.tags.clone_from(&room_info.deref().1);

        if let Some(alias) = &alias {
            store.application.names.insert(alias.to_string(), room_id.to_owned());
        }

        GenericChatItem { room_info, name, alias, is_dm, unread }
    }

    /// Get the room reference
    #[inline]
    fn room(&self) -> &MatrixRoom {
        &self.room_info.deref().0
    }

    /// Get the tags for this room
    #[inline]
    fn tags(&self) -> &Option<Tags> {
        &self.room_info.deref().1
    }
}

impl RoomLikeItem for GenericChatItem {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn alias(&self) -> Option<&RoomAliasId> {
        self.alias.as_deref()
    }

    fn room_id(&self) -> &RoomId {
        self.room().room_id()
    }

    fn has_tag(&self, tag: TagName) -> bool {
        if let Some(tags) = &self.room_info.deref().1 {
            tags.contains_key(&tag)
        } else {
            false
        }
    }

    fn recent_ts(&self) -> Option<&MessageTimeStamp> {
        self.unread.latest()
    }

    fn is_unread(&self) -> bool {
        self.unread.is_unread()
    }
}

impl Display for GenericChatItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let prefix = if self.is_dm { "[DM] " } else { "" };
        let unread = if self.is_unread() { "* " } else { "" };
        write!(f, "{}{}{}", prefix, unread, self.name)
    }
}

/// Item for room lists
#[derive(Debug, Clone)]
pub struct RoomItem {
    /// Room information
    room_info: MatrixRoomInfo,
    /// Room name
    name: String,
    /// Room alias
    alias: Option<OwnedRoomAliasId>,
    /// Unread message information
    unread: UnreadInfo,
}

impl RoomItem {
    /// Create a new room item
    fn new(room_info: MatrixRoomInfo, store: &mut ProgramStore) -> Self {
        let room = &room_info.deref().0;
        let room_id = room.room_id();

        let info = store.application.rooms.get_or_default(room_id.to_owned());
        let name = info.name.clone().unwrap_or_default();
        let alias = room.canonical_alias();
        let unread = info.unreads(&store.application.settings);
        info.tags.clone_from(&room_info.deref().1);

        if let Some(alias) = &alias {
            store.application.names.insert(alias.to_string(), room_id.to_owned());
        }

        RoomItem { room_info, name, alias, unread }
    }

    /// Get the room reference
    #[inline]
    fn room(&self) -> &MatrixRoom {
        &self.room_info.deref().0
    }

    /// Get the tags for this room
    #[inline]
    fn tags(&self) -> &Option<Tags> {
        &self.room_info.deref().1
    }
}

impl RoomLikeItem for RoomItem {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn alias(&self) -> Option<&RoomAliasId> {
        self.alias.as_deref()
    }

    fn room_id(&self) -> &RoomId {
        self.room().room_id()
    }

    fn has_tag(&self, tag: TagName) -> bool {
        if let Some(tags) = &self.room_info.deref().1 {
            tags.contains_key(&tag)
        } else {
            false
        }
    }

    fn recent_ts(&self) -> Option<&MessageTimeStamp> {
        self.unread.latest()
    }

    fn is_unread(&self) -> bool {
        self.unread.is_unread()
    }
}

impl Display for RoomItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let unread = if self.is_unread() { "* " } else { "" };
        write!(f, "{}{}", unread, self.name)
    }
}

/// Item for direct message lists
#[derive(Debug, Clone)]
pub struct DirectItem {
    /// Room information
    room_info: MatrixRoomInfo,
    /// Room name
    name: String,
    /// Room alias
    alias: Option<OwnedRoomAliasId>,
    /// Unread message information
    unread: UnreadInfo,
}

impl DirectItem {
    /// Create a new direct message item
    fn new(room_info: MatrixRoomInfo, store: &mut ProgramStore) -> Self {
        let room_id = room_info.0.room_id().to_owned();
        let alias = room_info.0.canonical_alias();

        let info = store.application.rooms.get_or_default(room_id);
        let name = info.name.clone().unwrap_or_default();
        let unread = info.unreads(&store.application.settings);
        info.tags.clone_from(&room_info.deref().1);

        DirectItem { room_info, name, alias, unread }
    }

    /// Get the room reference
    #[inline]
    fn room(&self) -> &MatrixRoom {
        &self.room_info.deref().0
    }

    /// Get the tags for this room
    #[inline]
    fn tags(&self) -> &Option<Tags> {
        &self.room_info.deref().1
    }
}

impl RoomLikeItem for DirectItem {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn alias(&self) -> Option<&RoomAliasId> {
        self.alias.as_deref()
    }

    fn has_tag(&self, tag: TagName) -> bool {
        if let Some(tags) = &self.room_info.deref().1 {
            tags.contains_key(&tag)
        } else {
            false
        }
    }

    fn room_id(&self) -> &RoomId {
        self.room().room_id()
    }

    fn recent_ts(&self) -> Option<&MessageTimeStamp> {
        self.unread.latest()
    }

    fn is_unread(&self) -> bool {
        self.unread.is_unread()
    }
}

impl Display for DirectItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let unread = if self.is_unread() { "* " } else { "" };
        write!(f, "{}{}", unread, self.name)
    }
}

/// Item for space lists
#[derive(Debug, Clone)]
pub struct SpaceItem {
    /// Room information
    room_info: MatrixRoomInfo,
    /// Space name
    name: String,
    /// Space alias
    alias: Option<OwnedRoomAliasId>,
}

impl SpaceItem {
    /// Create a new space item
    fn new(room_info: MatrixRoomInfo, store: &mut ProgramStore) -> Self {
        let room_id = room_info.0.room_id();
        let name = store
            .application
            .get_room_info(room_id.to_owned())
            .name
            .clone()
            .unwrap_or_default();
        let alias = room_info.0.canonical_alias();

        if let Some(alias) = &alias {
            store.application.names.insert(alias.to_string(), room_id.to_owned());
        }

        SpaceItem { room_info, name, alias }
    }

    /// Get the room reference
    #[inline]
    fn room(&self) -> &MatrixRoom {
        &self.room_info.deref().0
    }
}

impl RoomLikeItem for SpaceItem {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn room_id(&self) -> &RoomId {
        self.room().room_id()
    }

    fn alias(&self) -> Option<&RoomAliasId> {
        self.alias.as_deref()
    }

    fn has_tag(&self, _: TagName) -> bool {
        // Spaces typically don't have tags in client UI
        false
    }

    fn recent_ts(&self) -> Option<&MessageTimeStamp> {
        // We don't track timestamp for spaces yet
        None
    }

    fn is_unread(&self) -> bool {
        // We don't track unread status for spaces yet
        false
    }
}

impl Display for SpaceItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[SPACE] {}", self.name)
    }
}

/// Item for verification lists
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct VerifyItem {
    /// User device identifier
    user_dev: String,
    /// SAS verification object
    sasv1: SasVerification,
}

impl VerifyItem {
    /// Create a new verification item
    fn new(user_dev: String, sasv1: SasVerification) -> Self {
        VerifyItem { user_dev, sasv1 }
    }
    
    /// Get formatted status information
    fn show_item(&self) -> String {
        let state = if self.sasv1.is_done() {
            "done"
        } else if self.sasv1.is_cancelled() {
            "cancelled"
        } else if self.sasv1.emoji().is_some() {
            "accepted"
        } else {
            "not accepted"
        };

        if self.sasv1.is_self_verification() {
            let device = self.sasv1.other_device();

            if let Some(display_name) = device.display_name() {
                format!("Device verification with {display_name} ({state})")
            } else {
                format!("Device verification with device {} ({})", device.device_id(), state)
            }
        } else {
            format!("User Verification with {} ({})", self.sasv1.other_user_id(), state)
        }
    }
}

impl From<(&String, &SasVerification)> for VerifyItem {
    fn from((user_dev, sasv1): (&String, &SasVerification)) -> Self {
        VerifyItem::new(user_dev.clone(), sasv1.clone())
    }
}

impl Display for VerifyItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.show_item())
    }
}

// Type aliases for the specific list states
pub type DirectListState = ListState<DirectItem>;
pub type MemberListState = ListState<MemberItem>;
pub type RoomListState = ListState<RoomItem>;
pub type ChatListState = ListState<GenericChatItem>;
pub type UnreadListState = ListState<GenericChatItem>;
pub type SpaceListState = ListState<SpaceItem>;
pub type VerifyListState = ListState<VerifyItem>;

/// Common editor command handling for list states
impl<T> ListState<T> {
    /// Handle editor commands for the list
    pub fn editor_command(
        &mut self,
        act: &EditorAction,
        ctx: &ProgramContext,
        _store: &mut ProgramStore,
    ) -> ActionResult {
        match act {
            EditorAction::Movement(MovementAction::Down) => {
                self.select_next(false);
                Ok(vec![])
            },
            EditorAction::Movement(MovementAction::Up) => {
                self.select_prev(false);
                Ok(vec![])
            },
            EditorAction::Movement(MovementAction::First) => {
                self.select_first();
                Ok(vec![])
            },
            EditorAction::Movement(MovementAction::Last) => {
                self.select_last();
                Ok(vec![])
            },
            EditorAction::Movement(MovementAction::PageDown) => {
                if self.height > 0 {
                    for _ in 0..self.height.min(10) {
                        self.select_next(false);
                    }
                } else {
                    self.select_next(false);
                }
                Ok(vec![])
            },
            EditorAction::Movement(MovementAction::PageUp) => {
                if self.height > 0 {
                    for _ in 0..self.height.min(10) {
                        self.select_prev(false);
                    }
                } else {
                    self.select_prev(false);
                }
                Ok(vec![])
            },
            _ => Ok(vec![]),
        }
    }

    /// Handle jump commands for the list
    pub fn jump(
        &mut self,
        list: PositionList,
        dir: MoveDir1D,
        count: usize,
        _ctx: &ProgramContext,
    ) -> maxtryxResult<usize> {
        // Handle different jump lists and directions
        match (list, dir) {
            (PositionList::Jump, MoveDir1D::Next) => {
                for _ in 0..count {
                    self.select_next(false);
                }
                Ok(count)
            },
            (PositionList::Jump, MoveDir1D::Previous) => {
                for _ in 0..count {
                    self.select_prev(false);
                }
                Ok(count)
            },
            _ => Ok(0),
        }
    }

    /// Handle scroll commands for the list
    pub fn scroll(
        &mut self,
        style: &ScrollStyle,
        _ctx: &ProgramContext,
        _store: &mut ProgramStore,
    ) -> ActionResult {
        match style {
            ScrollStyle::Up(n) => {
                self.scroll_up(*n);
                Ok(vec![])
            },
            ScrollStyle::Down(n) => {
                self.scroll_down(*n);
                Ok(vec![])
            },
            ScrollStyle::Home => {
                self.scroll_to_top();
                Ok(vec![])
            },
            ScrollStyle::End => {
                self.scroll_to_bottom();
                Ok(vec![])
            },
            ScrollStyle::Page(dir) => {
                match dir {
                    MoveDir1D::Previous => self.scroll_up(self.height.min(10)),
                    MoveDir1D::Next => self.scroll_down(self.height.min(10)),
                }
                Ok(vec![])
            },
            _ => Ok(vec![]),
        }
    }

    /// Get the cursor position
    pub fn get_term_cursor(&self) -> Option<(u16, u16)> {
        None // Lists don't have a cursor position
    }

    /// Get completions from the list
    pub fn get_completions(&self) -> Option<Vec<String>> {
        None // Lists don't have completions by default
    }

    /// Get the word at the cursor
    pub fn get_cursor_word(&self, _style: &crate::modal::WordStyle) -> Option<String> {
        None // Lists don't have a cursor
    }

    /// Get the selected word
    pub fn get_selected_word(&self) -> Option<String> {
        None // Override in specific implementations
    }
}

// Placeholder implementation for MemberItem - will be replaced with actual implementation
#[derive(Clone)]
pub struct MemberItem {
    member: RoomMember,
    room_id: OwnedRoomId,
}

impl MemberItem {
    fn new(member: RoomMember, room_id: OwnedRoomId) -> Self {
        Self { member, room_id }
    }
}

// Additional implementations will be added for specific window types and list items