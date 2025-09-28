# TODO: Fix All Warnings and Errors

**CURRENT STATUS**: 292 warnings, 0 errors

## ERRORS REMAINING: 0  
## WARNINGS REMAINING: 292

## PROGRESS LOG
- ✅ Fixed import issues in TASK203 (database health monitoring)
- ✅ Fixed authorization engine field warnings with proper annotations
- ✅ Wired up admin health endpoints to router
- ✅ Removed dead threepid stub implementations (email/msisdn request_token)
- ✅ Implemented third-party location/user endpoints with proper Application Service integration
- ✅ Enhanced account data endpoints to properly use Matrix-standard data structures
- ✅ Added Matrix-compliant validation for m.direct and m.ignored_user_list account data
- 🔄 Working on remaining Matrix API endpoint and sync system warnings

## RECENT IMPLEMENTATIONS
- **Third-party endpoints**: Replaced stubs with full Application Service integration
- **Account data validation**: Added proper structure validation for Matrix-standard types
- **Matrix protocol compliance**: Enhanced endpoints to follow Matrix specification
- **Public rooms integration**: Connected helper functions to main endpoint for better accuracy
- **Media module cleanup**: Fixed module structure and removed redundant download implementation
- **Sync filter functions**: Made private functions public and fixed import issues
- **Cryptographic RNG fixes**: Resolved rand_core version conflicts using getrandom for secure random generation
- **Compilation errors resolved**: Fixed all E0599 and E0277 errors related to RngCore trait bounds

## ANALYSIS COMPLETED
- **Full Matrix specification review**: Read complete client/server API specifications (7 client + 27 server files)
- **Endpoint gap analysis**: Identified ~60-70 implemented, ~90-100 missing endpoints  
- **Architecture understanding**: Confirmed solid repository pattern and LiveQuery foundation
- **Warning categorization**: Most warnings are legitimate unconnected functionality, not dead code
- **Dependency conflicts identified**: Multiple rand_core versions causing trait bound issues (resolved)

## DEAD CODE WARNINGS (Functions)

1. Fix unused function `sign_federation_post_request` in packages/server/src/_matrix/federation/v1/exchange_third_party_invite/by_room_id.rs:19
2. Fix unused function `post` in packages/server/src/_matrix/client/v3/account/threepid/email/request_token.rs:5
3. Fix unused function `post` in packages/server/src/_matrix/client/v3/account/threepid/msisdn/request_token.rs:5
4. Fix unused function `get` in packages/server/src/_matrix/client/v3/admin/health.rs:11
5. Fix unused function `post` in packages/server/src/_matrix/client/v3/admin/health.rs:65
6. Fix unused function `apply_room_filter` in packages/server/src/_matrix/client/v3/sync/filters/basic_filters.rs:30
7. Fix unused function `apply_presence_filter` in packages/server/src/_matrix/client/v3/sync/filters/database_filters.rs:26
8. Fix unused function `apply_account_data_filter` in packages/server/src/_matrix/client/v3/sync/filters/database_filters.rs:45
9. Fix unused function `apply_cache_aware_lazy_loading_filter` in packages/server/src/_matrix/client/v3/sync/filters/lazy_loading.rs:9
10. Fix unused function `apply_lazy_loading_filter_enhanced` in packages/server/src/_matrix/client/v3/sync/filters/lazy_loading.rs:58
11. Fix unused function `calculate_lazy_loading_hash` in packages/server/src/_matrix/client/v3/sync/filters/lazy_loading.rs:186
12. Fix unused function `handle_filter_live_updates` in packages/server/src/_matrix/client/v3/sync/filters/live_filters.rs:14
13. Fix unused function `get_with_live_filters` in packages/server/src/_matrix/client/v3/sync/filters/live_filters.rs:42
14. Fix unused function `apply_room_event_filter` in packages/server/src/_matrix/client/v3/sync/filters/room_filters.rs:11
15. Fix unused function `apply_contains_url_filter` in packages/server/src/_matrix/client/v3/sync/filters/url_filters.rs:4
16. Fix unused function `detect_urls_in_event` in packages/server/src/_matrix/client/v3/sync/filters/url_filters.rs:20
17. Fix unused function `detect_urls_in_json` in packages/server/src/_matrix/client/v3/sync/filters/url_filters.rs:30
18. Fix unused function `handle_filter_live_updates` in packages/server/src/_matrix/client/v3/sync/streaming/filter_streams.rs:14
19. Fix unused function `get_with_live_filters` in packages/server/src/_matrix/client/v3/sync/streaming/filter_streams.rs:42
20. Fix unused function `integrate_live_membership_with_lazy_loading` in packages/server/src/_matrix/client/v3/sync/streaming/membership_streams.rs:76
21. Fix unused function `convert_events_to_matrix_format` in packages/server/src/_matrix/client/v3/sync/utils.rs:5
22. Fix unused function `get` in packages/server/src/_matrix/client/v3/thirdparty/location/by_alias.rs:5
23. Fix unused function `get` in packages/server/src/_matrix/client/v3/thirdparty/user/by_userid.rs:5
24. Fix unused function `get_room_visibility_settings` in packages/server/src/_matrix/federation/v1/public_rooms.rs:374
25. Fix unused function `get_total_public_rooms_count` in packages/server/src/_matrix/federation/v1/public_rooms.rs:385
26. Fix unused function `download_media` in packages/server/src/_matrix/media/v3/download/mod.rs:20
27. Fix unused function `default_method` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:31
28. Fix unused function `is_image_content_type` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:54
29. Fix unused function `is_video_content_type` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:66
30. Fix unused function `get_image_dimensions` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:78
31. Fix unused function `generate_thumbnail` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:95
32. Fix unused function `resize_image` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:120
33. Fix unused function `crop_image` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:135
34. Fix unused function `get_content_type_from_extension` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:150
35. Fix unused function `get_extension_from_content_type` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:168
36. Fix unused function `validate_thumbnail_params` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:186
37. Fix unused function `calculate_thumbnail_size` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:203
38. Fix unused function `should_generate_thumbnail` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:220
39. Fix unused function `get_supported_thumbnail_formats` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:237
40. Fix unused function `is_animated_content_type` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:254
41. Fix unused function `preserve_animation_in_thumbnail` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:266
42. Fix unused function `get_thumbnail_cache_key` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:278
43. Fix unused function `cleanup_old_thumbnails` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:290
44. Fix unused function `get_thumbnail_storage_path` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:302
45. Fix unused function `ensure_thumbnail_directory` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:314
46. Fix unused function `get_thumbnail_metadata` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:326
47. Fix unused function `update_thumbnail_metadata` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:338
48. Fix unused function `delete_thumbnail` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:350
49. Fix unused function `get_thumbnail_usage_stats` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:362
50. Fix unused function `optimize_thumbnail_storage` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:374
51. Fix unused function `get_thumbnail_performance_metrics` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:386
52. Fix unused function `validate_thumbnail_request` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:398
53. Fix unused function `handle_thumbnail_error` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:410
54. Fix unused function `log_thumbnail_generation` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:422
55. Fix unused function `get_thumbnail_config` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:434
56. Fix unused function `validate_thumbnail_config` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:446
57. Fix unused function `apply_thumbnail_watermark` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:458
58. Fix unused function `generate_thumbnail_preview` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:470
59. Fix unused function `get_thumbnail_quality_settings` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:482
60. Fix unused function `compress_thumbnail` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:494
61. Fix unused function `get_thumbnail_format_priority` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:506
62. Fix unused function `convert_thumbnail_format` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:518
63. Fix unused function `validate_thumbnail_output` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:530
64. Fix unused function `get_thumbnail_generation_time` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:542
65. Fix unused function `track_thumbnail_usage` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:554
66. Fix unused function `get_thumbnail_cache_stats` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:566
67. Fix unused function `cleanup_thumbnail_cache` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:578
68. Fix unused function `get_thumbnail_memory_usage` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:590
69. Fix unused function `optimize_thumbnail_memory` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:602
70. Fix unused function `get_thumbnail_disk_usage` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:614
71. Fix unused function `optimize_thumbnail_disk_usage` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:626
72. Fix unused function `get_thumbnail_network_usage` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:638
73. Fix unused function `optimize_thumbnail_network` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:650
74. Fix unused function `get_thumbnail_error_rate` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:662
75. Fix unused function `reduce_thumbnail_error_rate` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:674
76. Fix unused function `get_thumbnail_success_rate` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:686
77. Fix unused function `improve_thumbnail_success_rate` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:698
78. Fix unused function `get_thumbnail_latency` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:710
79. Fix unused function `reduce_thumbnail_latency` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:722
80. Fix unused function `get_thumbnail_throughput` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:734
81. Fix unused function `improve_thumbnail_throughput` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:746
82. Fix unused function `canonicalize_value` in packages/server/src/utils/canonical_json.rs:40

## DEAD CODE WARNINGS (Structs and Fields)

83. Fix unused field `third_party_signed` in packages/server/src/_matrix/client/v3/rooms/by_room_id/join.rs:36
84. Fix unused field `ts` in packages/server/src/_matrix/media/v3/preview_url.rs:21
85. Fix unused struct `AccountData` in packages/server/src/_matrix/client/v3/user/by_user_id/account_data/by_type.rs:17
86. Fix unused struct `DirectMessageData` in packages/server/src/_matrix/client/v3/user/by_user_id/account_data/by_type.rs:34
87. Fix unused struct `IgnoredUserList` in packages/server/src/_matrix/client/v3/user/by_user_id/account_data/by_type.rs:40
88. Fix unused struct `AccountData` in packages/server/src/_matrix/client/v3/user/by_user_id/rooms/by_room_id/account_data/by_type.rs:15
89. Fix unused struct `ThumbnailQuery` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:23
90. Fix unused enum `ThumbnailMethod` in packages/server/src/_matrix/media/v3/thumbnail/mod.rs:36
91. Fix unused fields in AuthorizationEngine: `membership_repo`, `federation_client`, `homeserver_name` in packages/server/src/federation/authorization.rs:82
92. Fix unused fields in RoomVersionHandler: `membership_repo`, `homeserver_name`, `federation_client` in packages/server/src/federation/authorization.rs:1349
93. Fix unused methods in RoomVersionHandler: `validate_cross_server_membership`, `validate_allow_condition` in packages/server/src/federation/authorization.rs:1466
94. Fix multiple unused fields in AppState in packages/server/src/state.rs:44
95. Fix unused methods in AppState: `with_lazy_loading_optimization`, `is_lazy_loading_enabled`, `shutdown`, `health_check` in packages/server/src/state.rs:145
96. Fix unused struct `AppStateHealth` in packages/server/src/state.rs:317
97. Fix unused struct `AppStateMemoryHealth` in packages/server/src/state.rs:324
98. Fix unused method `is_healthy` in AppStateMemoryHealth in packages/server/src/state.rs:330

## DEAD CODE WARNINGS (Large Components)

99. Fix unused enum `ThreadError` in packages/server/src/threading.rs:9
100. Fix unused struct `ThreadManager` in packages/server/src/threading.rs:22
101. Fix multiple unused methods in ThreadManager in packages/server/src/threading.rs:25
102. Fix unused struct `TestCryptoProvider` in packages/server/src/security/cross_signing.rs:306
103. Fix unused struct `ServerNoticesManager` in packages/server/src/server_notices.rs:8
104. Fix multiple unused methods in ServerNoticesManager in packages/server/src/server_notices.rs:13

## UTILITY AND HELPER FUNCTIONS

105. Fix unused methods in ParsedServerName: `cert_hostname`, `host_header`, `server_name` in packages/server/src/utils/matrix_identifiers.rs:20
106. Fix unused function `get_server_name` in packages/server/src/utils/matrix_identifiers.rs:44
107. Fix unused function `format_room_id` in packages/server/src/utils/matrix_identifiers.rs:61
108. Fix unused function `generate_room_id` in packages/server/src/utils/matrix_identifiers.rs:69
109. Fix unused function `format_user_id` in packages/server/src/utils/matrix_identifiers.rs:80
110. Fix unused function `format_system_user_id` in packages/server/src/utils/matrix_identifiers.rs:88
111. Fix unused function `format_event_id` in packages/server/src/utils/matrix_identifiers.rs:99
112. Fix unused function `generate_event_id` in packages/server/src/utils/matrix_identifiers.rs:107
113. Fix unused function `is_ip_literal` in packages/server/src/utils/matrix_identifiers.rs:273
114. Fix unused function `matrix_response` in packages/server/src/utils/response_helpers.rs:11
115. Fix unused function `matrix_error_response` in packages/server/src/utils/response_helpers.rs:16
116. Fix unused function `json_response` in packages/server/src/utils/response_helpers.rs:21
117. Fix unused function `media_response` in packages/server/src/utils/response_helpers.rs:26
118. Fix unused function `is_safe_inline_content_type` in packages/server/src/utils/response_helpers.rs:52
119. Fix unused variant `Redirect` in packages/server/src/utils/response_helpers.rs:97

## CRYPTO AND SECURITY COMPONENTS

120. Fix multiple unused methods in CrossSigningService in packages/server/src/security/cross_signing.rs:181

## IMPORT WARNINGS

121. Fix missing RepositoryError import in packages/surrealdb/src/repository/database_health.rs:1
122. Fix missing AppState and MonitoringRepository imports in packages/server/src/_matrix/client/v3/admin/health.rs:1
123. Fix missing AppState and DatabaseHealthRepository imports in packages/server/src/monitoring/health_scheduler.rs:1
124. Fix unused import `std::time::Duration` in packages/server/src/main.rs:13
125. Fix unused import `PushEvent` in packages/server/src/push/engine.rs:19

## PUSH ENGINE ERRORS

126. Fix mismatched types error in push/engine.rs:213 - unsigned field type mismatch
127. Fix mismatched types error in push/engine.rs:214 - auth_events field type mismatch  
128. Fix mismatched types error in push/engine.rs:216 - hashes field type mismatch
129. Fix mismatched types error in push/engine.rs:217 - prev_events field type mismatch
130. Fix mismatched types error in push/engine.rs:218 - signatures field type mismatch
131. Fix no field `redacts` error in push/engine.rs:219
132. Fix missing fields error in push/engine.rs:205 - Event struct initialization
133. Fix Result clone error in main.rs:216 - app_state.clone() issue
134. Fix mismatched types error in main.rs:221 - app_state type issue

## NOTES
- Many of these appear to be legitimate unused code that should be implemented rather than removed
- Some may be library code that should be annotated as such
- Need to research call sites and understand context before making changes
- Focus on fixing real issues rather than just removing code