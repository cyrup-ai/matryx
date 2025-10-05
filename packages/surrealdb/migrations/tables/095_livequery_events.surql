-- =====================================================
-- Migration: 095
-- LiveQuery Event Infrastructure
-- Purpose: DEFINE EVENT statements for real-time sync
-- =====================================================

-- Global sequence function for ordering sync events
DEFINE FUNCTION fn::next_sequence() {
    -- Get current sequence number
    LET $current = (
        SELECT VALUE sequence_number
        FROM matrix_global_sequence:global
        LIMIT 1
    );

    -- Increment and return new sequence
    LET $next = $current + 1;

    UPDATE matrix_global_sequence:global
    SET sequence_number = $next,
        updated_at = time::now();

    RETURN $next;
};

-- PDU events for room timeline
DEFINE EVENT pdu_sync_event ON TABLE pdu WHEN $event = "CREATE" THEN (
    LET $sequence = fn::next_sequence();

    -- Find room members to notify
    LET $room_members = (
        SELECT user_id FROM room_membership
        WHERE room_id = $this.room_id
        AND membership = 'join'
    );

    -- Create sync records for each member
    FOR $member IN $room_members {
        CREATE matrix_sync_room_event SET
            user_id = $member.user_id,
            room_id = $this.room_id,
            event_id = $this.event_id,
            event_type = $this.event_type,
            sequence_number = $sequence,
            timeline_event = {
                event_id: $this.event_id,
                type: $this.event_type,
                sender: $this.sender,
                content: $this.content,
                origin_server_ts: $this.origin_server_ts,
                unsigned: $this.unsigned
            },
            state_event = {},
            created_at = time::now()
    }
);

-- State events for room state
DEFINE EVENT state_sync_event ON TABLE event
WHEN $event = "CREATE" AND $this.state_key != NONE THEN (
    LET $sequence = fn::next_sequence();

    -- Find room members to notify
    LET $room_members = (
        SELECT user_id FROM room_membership
        WHERE room_id = $this.room_id
        AND membership = 'join'
    );

    -- Create sync records for each member
    FOR $member IN $room_members {
        CREATE matrix_sync_room_event SET
            user_id = $member.user_id,
            room_id = $this.room_id,
            event_id = $this.event_id,
            event_type = $this.event_type,
            sequence_number = $sequence,
            timeline_event = {},
            state_event = {
                event_id: $this.event_id,
                type: $this.event_type,
                sender: $this.sender,
                state_key: $this.state_key,
                content: $this.content,
                origin_server_ts: $this.origin_server_ts
            },
            created_at = time::now()
    }
);

-- Account data events for sync
DEFINE EVENT account_data_sync_event ON TABLE account_data
WHEN $event = "CREATE" OR $event = "UPDATE" THEN (
    LET $sequence = fn::next_sequence();

    CREATE matrix_sync_account_data SET
        user_id = $this.user_id,
        room_id = $this.room_id,
        data_type = $this.data_type,
        sequence_number = $sequence,
        content = $this.content,
        created_at = time::now()
);

-- Presence events for sync
DEFINE EVENT presence_sync_event ON TABLE user_presence_update
WHEN $event = "CREATE" OR $event = "UPDATE" THEN (
    LET $sequence = fn::next_sequence();

    -- Find users in shared rooms (presence subscribers)
    LET $shared_room_users = (
        SELECT DISTINCT rm2.user_id
        FROM room_membership rm1
        JOIN room_membership rm2 ON rm1.room_id = rm2.room_id
        WHERE rm1.user_id = $this.user_id
        AND rm2.user_id != $this.user_id
        AND rm1.membership = 'join'
        AND rm2.membership = 'join'
    );

    -- Create sync records for subscribers
    FOR $subscriber IN $shared_room_users {
        CREATE matrix_sync_presence SET
            user_id = $subscriber.user_id,
            presence_user_id = $this.user_id,
            sequence_number = $sequence,
            presence = $this.presence,
            last_active_ago = $this.last_active_ago,
            status_msg = $this.status_msg,
            currently_active = $this.currently_active,
            created_at = time::now()
    }
);

-- Device list events for sync
DEFINE EVENT device_sync_event ON TABLE device
WHEN $event = "CREATE" OR $event = "UPDATE" THEN (
    LET $sequence = fn::next_sequence();

    -- Find users in shared rooms (E2EE participants)
    LET $shared_room_users = (
        SELECT DISTINCT rm2.user_id
        FROM room_membership rm1
        JOIN room_membership rm2 ON rm1.room_id = rm2.room_id
        WHERE rm1.user_id = $this.user_id
        AND rm1.membership = 'join'
        AND rm2.membership = 'join'
    );

    -- Create sync records for E2EE participants
    FOR $participant IN $shared_room_users {
        CREATE matrix_sync_device_list SET
            user_id = $participant.user_id,
            changed_user_id = $this.user_id,
            sequence_number = $sequence,
            device_id = $this.device_id,
            created_at = time::now()
    }
);

-- To-device message events for sync
DEFINE EVENT to_device_sync_event ON TABLE to_device_messages
WHEN $event = "CREATE" THEN (
    LET $sequence = fn::next_sequence();

    CREATE matrix_sync_to_device SET
        user_id = $this.recipient_id,
        device_id = $this.device_id,
        sequence_number = $sequence,
        sender = $this.sender_id,
        message_type = $this.message_type,
        content = $this.content,
        created_at = time::now()
);

-- Typing notification events for ephemeral sync
DEFINE EVENT typing_sync_event ON TABLE typing_notification
WHEN $event = "CREATE" THEN (
    LET $sequence = fn::next_sequence();

    -- Find room members to notify (exclude typing user)
    LET $room_members = (
        SELECT user_id FROM room_membership
        WHERE room_id = $this.room_id
        AND membership = 'join'
        AND user_id != $this.user_id
    );

    -- Create ephemeral sync records
    FOR $member IN $room_members {
        CREATE matrix_sync_room_event SET
            user_id = $member.user_id,
            room_id = $this.room_id,
            event_id = NONE,
            event_type = 'm.typing',
            sequence_number = $sequence,
            timeline_event = {
                type: 'm.typing',
                content: {
                    user_id: $this.user_id,
                    typing: $this.typing
                }
            },
            state_event = {},
            created_at = time::now()
    }
);

-- Read receipt events for ephemeral sync
DEFINE EVENT receipt_sync_event ON TABLE receipts
WHEN $event = "CREATE" OR $event = "UPDATE" THEN (
    LET $sequence = fn::next_sequence();

    -- Find room members to notify
    LET $room_members = (
        SELECT user_id FROM room_membership
        WHERE room_id = $this.room_id
        AND membership = 'join'
    );

    -- Create ephemeral sync records
    FOR $member IN $room_members {
        CREATE matrix_sync_room_event SET
            user_id = $member.user_id,
            room_id = $this.room_id,
            event_id = $this.event_id,
            event_type = 'm.receipt',
            sequence_number = $sequence,
            timeline_event = {
                type: 'm.receipt',
                content: {
                    event_id: $this.event_id,
                    receipt_type: $this.receipt_type,
                    user_id: $this.user_id,
                    ts: $this.timestamp,
                    thread_id: $this.thread_id
                }
            },
            state_event = {},
            created_at = time::now()
    }
);
