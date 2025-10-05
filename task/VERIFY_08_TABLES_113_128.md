# VERIFY Task 08: Tables 113-128 (NEW P1-P2 Priority)

## Tables to Verify (16 tables) - NEWLY CREATED
- 113_transaction.surql 🔸 P1
- 114_transaction_dedupe.surql 🔸 P1
- 115_transaction_mapping.surql 🔸 P1
- 116_room_capabilities.surql 🔸 P1
- 117_user_capabilities.surql 🔸 P1
- 118_user_relationships.surql 🔹 P2
- 119_user_presence.surql 🔹 P2
- 120_third_party_identifiers.surql 🔹 P2
- 121_third_party_invite_log.surql 🔹 P2
- 122_openid_tokens.surql 🔹 P2
- 123_oauth.surql 🔹 P2
- 124_registration_token.surql 🔹 P2
- 125_registration_attempt.surql 🔹 P2
- 126_captcha_challenges.surql 🔹 P2
- 127_uia_sessions.surql 🔹 P2
- 128_event_reports.surql 🔹 P2

## Critical Verification Points
- Transaction tables: Idempotency for event creation
- Capabilities: Authorization and feature flags
- UIA sessions: Multi-stage authentication flows
- Event reports: Content moderation
