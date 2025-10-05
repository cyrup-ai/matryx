use chrono::Utc;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::federation::client::{FederationClient, FederationClientError};
use matryx_entity::types::{EDU, PDU, Transaction};

/// Event to send to another homeserver
#[derive(Debug, Clone)]
pub enum OutboundEvent {
    Pdu { destination: String, pdu: Box<PDU> },
    Edu { destination: String, edu: Box<EDU> },
}

/// In-flight transaction tracker
struct InFlightTransaction {
    txn_id: String,
    sent_at: Instant,
    retry_count: usize,
}

/// Outbound transaction queue with per-destination batching
pub struct OutboundTransactionQueue {
    /// PDU queues per destination
    pdu_queues: HashMap<String, VecDeque<PDU>>,
    /// EDU queues per destination  
    edu_queues: HashMap<String, VecDeque<EDU>>,
    /// Channel for receiving events to send
    event_rx: mpsc::UnboundedReceiver<OutboundEvent>,
    /// Federation client for HTTP requests
    federation_client: Arc<FederationClient>,
    /// Origin server name (our homeserver)
    origin: String,
    /// Maximum PDUs per transaction (Matrix spec: 50)
    max_pdus_per_txn: usize,
    /// Maximum EDUs per transaction (Matrix spec: 100)
    max_edus_per_txn: usize,
    /// Currently in-flight transactions per destination
    in_flight_transactions: HashMap<String, InFlightTransaction>,
}

impl OutboundTransactionQueue {
    pub fn new(
        event_rx: mpsc::UnboundedReceiver<OutboundEvent>,
        federation_client: Arc<FederationClient>,
        origin: String,
    ) -> Self {
        Self {
            pdu_queues: HashMap::new(),
            edu_queues: HashMap::new(),
            event_rx,
            federation_client,
            origin,
            max_pdus_per_txn: 50,
            max_edus_per_txn: 100,
            in_flight_transactions: HashMap::new(),
        }
    }

    /// Run the queue processing loop
    pub async fn run(mut self) {
        info!("Starting outbound transaction queue");

        loop {
            tokio::select! {
                // Receive new events to queue
                Some(event) = self.event_rx.recv() => {
                    self.queue_event(event).await;
                }
                // Periodically flush queues (every 100ms)
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    self.flush_all_queues().await;
                }
            }
        }
    }

    async fn queue_event(&mut self, event: OutboundEvent) {
        match event {
            OutboundEvent::Pdu { destination, pdu } => {
                let queue = self.pdu_queues.entry(destination.clone()).or_default();
                queue.push_back(*pdu);

                // Immediate flush if queue is full
                if queue.len() >= self.max_pdus_per_txn {
                    debug!(
                        destination = %destination,
                        queue_size = queue.len(),
                        "PDU queue full, flushing immediately"
                    );
                    if let Err(e) = self.flush_queue(&destination).await {
                        error!(
                            destination = %destination,
                            error = ?e,
                            "Failed to flush full PDU queue"
                        );
                    }
                }
            },
            OutboundEvent::Edu { destination, edu } => {
                let queue = self.edu_queues.entry(destination.clone()).or_default();
                queue.push_back(*edu);

                if queue.len() >= self.max_edus_per_txn {
                    debug!(
                        destination = %destination,
                        queue_size = queue.len(),
                        "EDU queue full, flushing immediately"
                    );
                    if let Err(e) = self.flush_queue(&destination).await {
                        error!(
                            destination = %destination,
                            error = ?e,
                            "Failed to flush full EDU queue"
                        );
                    }
                }
            },
        }
    }

    async fn flush_all_queues(&mut self) {
        // Get all destinations with queued events
        let destinations: Vec<String> = self
            .pdu_queues
            .keys()
            .chain(self.edu_queues.keys())
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        for destination in destinations {
            if let Err(e) = self.flush_queue(&destination).await {
                error!(
                    destination = %destination,
                    error = ?e,
                    "Failed to flush transaction queue"
                );
            }
        }
    }

    async fn flush_queue(&mut self, destination: &str) -> Result<(), FederationClientError> {
        // Check if transaction already in flight (ordering enforcement)
        if let Some(in_flight) = self.in_flight_transactions.get(destination) {
            let elapsed = in_flight.sent_at.elapsed();
            debug!(
                destination = %destination,
                txn_id = %in_flight.txn_id,
                retry_count = in_flight.retry_count,
                elapsed_ms = elapsed.as_millis(),
                "Skipping flush: transaction already in flight"
            );
            return Ok(());
        }

        // Take up to max_pdus_per_txn PDUs
        let pdus: Vec<PDU> = self
            .pdu_queues
            .get_mut(destination)
            .map(|q| (0..self.max_pdus_per_txn).filter_map(|_| q.pop_front()).collect())
            .unwrap_or_default();

        // Take up to max_edus_per_txn EDUs
        let edus: Vec<EDU> = self
            .edu_queues
            .get_mut(destination)
            .map(|q| (0..self.max_edus_per_txn).filter_map(|_| q.pop_front()).collect())
            .unwrap_or_default();

        // Nothing to send
        if pdus.is_empty() && edus.is_empty() {
            return Ok(());
        }

        // Generate unique transaction ID
        let txn_id = format!("txn_{}", Uuid::new_v4());

        // Create transaction
        let transaction = Transaction::new(
            self.origin.clone(),
            Utc::now().timestamp_millis(),
            pdus.clone(),
            edus.clone(),
        );

        info!(
            destination = %destination,
            txn_id = %txn_id,
            pdu_count = pdus.len(),
            edu_count = edus.len(),
            "Sending transaction"
        );

        // Mark as in-flight
        self.in_flight_transactions.insert(
            destination.to_string(),
            InFlightTransaction {
                txn_id: txn_id.clone(),
                sent_at: Instant::now(),
                retry_count: 0,
            },
        );

        // Send with retry logic
        let result = self.send_transaction_with_retry(destination, &txn_id, transaction).await;

        // Remove from in-flight
        self.in_flight_transactions.remove(destination);

        result
    }

    async fn send_transaction_with_retry(
        &self,
        destination: &str,
        txn_id: &str,
        transaction: Transaction,
    ) -> Result<(), FederationClientError> {
        let max_retries = 5;
        let mut retry_count = 0;
        let mut backoff = Duration::from_millis(100);

        loop {
            match self
                .federation_client
                .send_transaction(destination, txn_id, &transaction)
                .await
            {
                Ok(response) => {
                    info!(
                        destination = %destination,
                        txn_id = %txn_id,
                        retry_count = retry_count,
                        "Transaction sent successfully"
                    );

                    // Log any PDU failures
                    for (event_id, result) in &response.pdus {
                        if let Some(error) = &result.error {
                            warn!(
                                destination = %destination,
                                event_id = %event_id,
                                error = %error,
                                "PDU rejected by destination"
                            );
                        }
                    }

                    return Ok(());
                },
                Err(FederationClientError::HttpError(_)) | Err(FederationClientError::Timeout)
                    if retry_count < max_retries =>
                {
                    retry_count += 1;
                    warn!(
                        destination = %destination,
                        txn_id = %txn_id,
                        retry_count = retry_count,
                        backoff_ms = backoff.as_millis(),
                        "Transaction send failed, retrying"
                    );

                    tokio::time::sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, Duration::from_secs(30));
                },
                Err(e) => {
                    error!(
                        destination = %destination,
                        txn_id = %txn_id,
                        retry_count = retry_count,
                        error = ?e,
                        "Transaction send failed"
                    );
                    return Err(e);
                },
            }
        }
    }
}
