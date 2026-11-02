# So bản Rust với bản Tauri (nghiệm thu groove-gd1_29)

Soát 02/11/2026 trên nhánh `claude/gd1_29`. Mỗi dòng đối chiếu một hành vi ở mục 1 hoặc một lỗi ở mục 3 của
`docs/rewrite-spec.md` với mã thật và test thật. Đường dẫn `src/…`, `src-tauri/…` trong đặc tả: sau khi gộp nhánh
`claude/remove-tauri` thì đọc qua tag `tauri-final` (`git -C groove show tauri-final:src-tauri/src/lib.rs`).

Trạng thái:

- **test**: có test tự động chạy qua đúng đường mã đó (tên test ghi kèm). Test view chạy trong cửa sổ ảo của GPUI, không thấy
  pixel, không ra loa.
- **chạy thật**: đã thấy trên app thật chạy trong desktop Win32 ẩn (`tools/measure-hidden.ps1`, bản debug và bản release có
  debug assertions, dữ liệu tạm, thư viện trống). Không phải màn thật: không có nền mờ, không con trỏ thật, không taskbar.
- **chỉ đọc mã**: đã đọc mã và đường nối, chưa có test bao trọn.
- **chưa kiểm**: cần máy, OS, thiết bị hay mắt người. Bước kiểm ở `plan/_backlog.md`.

Không dòng nào ghi "đạt": chủ dự án chưa nhìn app thật lần nào.

## Hành vi (đặc tả mục 1)

| Mục | Hành vi | Trạng thái | Bằng chứng / ghi chú |
| --- | --- | --- | --- |
| 1.1 | Cửa sổ không viền, nền mờ, 800 × 600, tiêu đề theo trang | test + chạy thật | `main_window_is_frameless_and_blurred`, `window_title_follows_route_and_override`; chạy thật thấy tiêu đề `My Music \| Groove X`, `Now Playing \| Groove X`. Nền mờ, chữ trên nền Blurred: chưa kiểm (desktop ẩn không có DWM) |
| 1.1 | Cột trái 230 / 56 px, lưu trạng thái thu, ba mục, Settings đáy | test + chạy thật | `toggling_side_nav_updates_settings`, `nav_click_navigates_and_persists`, `search_and_settings_emit_events`; ảnh chạy thật đúng bố cục |
| 1.1 | Thanh trên: lùi, ô tìm mở bảng lệnh, ba nút cửa sổ | test (lùi, ô tìm) / chưa kiểm (thu nhỏ, phóng to, kéo cửa sổ của OS) | `nav_click_navigates_and_persists`, `search_and_settings_emit_events` |
| 1.1 | Mở về trang cuối, không nhớ category | test | `opens_at_last_active_route`, `category_route_keeps_parent_highlight_and_is_not_saved` |
| 1.1 | Toast góc trên phải, có nút đóng | chỉ đọc mã | `ToastViewport` ở `app.rs`. Thời lượng 5 s của materiant, bản cũ 4 s (ghi `plan/confirm.md`) |
| 1.1 | Màu nhấn cyan, theme đổi được | test + chạy thật | `settings_change_applies_theme` (cyan `#06b6d4` nền tối); trước task này app chạy màu hồng mặc định của materiant, thấy khi chạy thật. Đổi theme qua `/theme` (`cli_commands_do_real_work`) |
| 1.2 | Lưới: hai thẻ hành động, tab Albums / Folders, 2-6 cột, ô Importing, rỗng có hướng dẫn | test + chạy thật | `loads_albums_folders_and_playlists`, `grid_measures_columns_within_limits`, `progress_drives_the_flags`; ảnh chạy thật màn rỗng |
| 1.2 | Thẻ: Play all, Delete hỏi lại, bấm vào danh sách, menu chuột phải | test | `click_opens_but_overlay_buttons_do_not`, `page_events_reach_dialogs_and_business`, `play_all_follows_the_reference_not_the_name`. Hover phóng bìa: bỏ có chủ ý (đặc tả mục 4) |
| 1.2 | Playlists: tạo tên + màu, rỗng toast, trùng báo | test | `page_events_reach_dialogs_and_business`, `playlist_name_errors_are_typed` |
| 1.2 | Danh sách bài: đầu dính thu ở 120 / mở dưới 15, bảng 4 cột, bấm dòng, chuột phải menu bài | test | `header_collapses_when_scrolled_and_expands_back`, `clicking_a_row_plays_it_with_the_whole_list`, `click_toggles_the_current_track_and_plays_others`, `right_click_reports_the_song`, `song_menu_opens_by_mouse_and_keyboard`, `artist_column_hides_below_768` |
| 1.2 | Thư viện đổi thì nạp lại | test | `reloads_on_change_and_reports_a_vanished_category`, `bulk_progress_and_moved_reload_the_grid` |
| 1.3 | Không bài: thanh mờ, không bấm được | test + chạy thật | `bar_without_a_song_is_inert` |
| 1.3 | Bìa, chữ sang Now Playing, marquee | chỉ đọc mã | `marquee`: `runs_only_when_text_is_wider_than_the_box` (hàm thuần). Bấm bìa chưa có test |
| 1.3 | Shuffle, Prev, Play, Next, Repeat | test | `transport_buttons_drive_the_player` (thêm ở task này). **Trước task này bốn nút Shuffle, Prev, Next, Repeat không có `on_click`**, bấm không làm gì |
| 1.3 | Thanh tua chia đoạn, tua khi thả | test | `dragging_seeks_once_on_release`, `click_matches_what_is_drawn_when_timeline_starts_late`, `label_names_the_segment_under_the_cursor` |
| 1.3 | Âm lượng: icon theo mức, bánh xe ±1, mute; nút Settings | test (icon, mute) / chỉ đọc mã (bánh xe, nút Settings) | `volume_icon_follows_the_thresholds`, `volume_and_mute_reach_the_engine` |
| 1.4 | Bảng hàng đợi, shuffle, repeat, khôi phục không tự phát | test | 40 test ở `groove-core/src/queue.rs`, `restore_loads_at_the_old_position_without_playing` |
| 1.5 | Now Playing ba cột, quầng màu, hộp đoạn bấm tua, hàng đợi bấm phát | test + chạy thật | `buttons_open_dialogs_and_queue_lists_tracks`, `clicking_a_queue_row_plays_it`, `active_segment_follows_the_position_and_jump_goes_to_its_start` (giờ đi qua đúng đường bấm dòng). Chạy thật thấy chữ trống bị cắt và cột lời chỉ ~86 px ở 800 px: đã sửa (chữ xuống dòng, hạ cỡ tối thiểu hai cột bên), nhìn lại ở màn có bài: chưa kiểm |
| 1.5 | Đổi bài thì đóng hộp đang sửa | test | `c4_changing_the_playing_track_closes_the_dialog` (lyrics, timeline); hộp metadata thêm ở task này trong `buttons_open_dialogs_and_queue_lists_tracks` |
| 1.6 | Timeline: định dạng, sắp xếp, lưu tệp + DB | test | 22 test `timeline.rs`, `c5_timeline_saves_the_raw_text_and_the_player_parses_it` |
| 1.7 | Lyrics: hộp 500 px, lưu tệp + DB | test | `c1_lyrics_open_reads_the_file_and_save_writes_file_and_db_then_refreshes` |
| 1.8 | Metadata: đọc từ tệp, ba tab, bìa, chế độ ghi, áp cho nhóm, Fix grouping | test | 16 test `metadata/dialog.rs`, 9 test `fields.rs`, 37 test `tags.rs`. Kéo thả tệp từ OS, hộp chọn tệp: chưa kiểm |
| 1.9 | Bảng lệnh: ba thao tác nhanh, tìm 250 ms, `/` lệnh, Tab, chọn bài không đóng | test + chạy thật | 13 test `command_palette.rs`, `quick_actions_all_run`, `cli_commands_do_real_work`; ảnh chạy thật mở bằng `Ctrl+K` |
| 1.10 | Settings: hai bước, EQ 10 dải, bass boost, preset, phím tắt JSON, Huỷ khôi phục | test + chạy thật | 13 test `settings/dialog.rs`; ảnh chạy thật mở bằng nút đáy cột trái. Nghe EQ: chưa kiểm |
| 1.10 | Phím tắt mặc định, chuỗi 800 ms, bỏ qua khi gõ | test + chạy thật | `default_keys_run_their_commands`, `sequence_times_out`, `text_input_blocks_all_but_ctrl_k`; chạy thật `g p`, `g m`, `ctrl-k`, `escape` đưa vào GPUI đổi đúng màn |
| 1.11 | Import, rescan, bìa nhóm, khoá album, gốc theo dõi | test | 22 test `tests/indexer_import.rs`, `import_goes_through_the_picker`. Hộp chọn thư mục thật của OS: chưa kiểm |
| 1.11 | Watcher, reconcile khi mở | test | 26 test `tests/watcher_reconcile.rs` |
| 1.11 | Watcher báo toast tóm tắt | test | `outside_changes_get_a_summary_toast` (thêm ở task này). **Trước task này không có toast nào** cho thay đổi ngoài app; thêm sự kiện `LibraryEvent::ExternalChange` |
| 1.11 | So đường dẫn theo đoạn, hoa thường theo OS | test | 10 test `paths.rs` |
| 1.12 | Liên kết sáu đuôi, một phiên, tệp ngoài phát | test (Windows) / chưa kiểm (macOS Finder, Linux) | 14 test `instance.rs`, 12 test `open_paths.rs`, 6 test `tests/packaging.rs` |
| 1.12 | Nút taskbar | test (cửa sổ ảo) / chưa kiểm (taskbar thật) | 8 test `taskbar_windows.rs`; trên desktop ẩn `taskbar list init: 0x80004001` là đúng (không có taskbar) |
| 1.13 | Khoá localStorage chuyển sang `settings.json`, `session.json` | test | 20 test `settings.rs`, 10 test `session.rs`. Đọc localStorage cũ (task 28, tuỳ chọn): chưa làm, `legacy_ls.rs` là tệp rỗng |

## Lỗi (đặc tả mục 3)

| Mã | Trạng thái | Bằng chứng / ghi chú |
| --- | --- | --- |
| N1 | test | `a_folder_that_cannot_be_read_keeps_its_songs_and_playlists`, `an_offline_root_is_left_alone_even_long_past_the_grace_period`, `only_a_long_confirmed_absence_is_purged`. "Người dùng xác nhận xoá": API `reconcile::forget_missing` có test, **không có đường vào từ UI**; dọn sau 30 ngày thì có (đặc tả ghi "hoặc"). Ghi `plan/confirm.md` |
| N2 | test | `timeline_survives_every_mode_on_every_format`, `lyrics_and_timeline_in_the_db_survive_a_reimport`, `clean_keeps_lyrics_and_timeline_on_every_format` |
| N3 | test | `renaming_onto_another_file_loses_nothing`, `names_with_a_path_never_leave_the_folder`, `failed_write_leaves_the_original_untouched_and_no_temp` |
| N4 | test | `renaming_a_file_while_closed_keeps_id_playlist_and_notes`, `a_reported_folder_rename_moves_the_tree` |
| N5 | test | `a_bad_file_is_skipped`, `three_errors_in_a_row_stop_playback`, `removed_tracks_leave_the_queue`, `failure_events_become_toasts` |
| N6 | test | `second_external_file_plays`, `n6_second_external_file_plays_and_the_end_does_not_stick` |
| N7 | test (thư viện) / chỉ đọc mã (UI) | `import_returns_at_once_and_reads_keep_flowing_while_the_writer_is_busy`, `reads_do_not_wait_for_an_open_write_transaction`. "UI thread không chạm đĩa" chỉ đọc mã; chạy thật 30 s trên thư viện trống không đứng |
| N8 | test | `next_on_the_last_track_really_stops`, `n8_next_at_the_end_without_repeat_stops_and_keeps_the_track` |
| N9 | test | `removed_folder_stays_out_after_a_rescan_and_keeps_files`, `removed_album_stays_out_after_import`, `c8_roots_tab_lists_adds_and_forgets_roots` |
| S1 | test | `same_named_albums_stay_apart_by_group_key` |
| S2 | test | `s2_playing_the_current_track_with_a_new_list_still_replaces_the_queue`, `play_all_replaces_the_queue_when_the_first_song_is_playing` |
| S3 | test | `play_all_follows_the_reference_not_the_name` |
| S4 | test | `s4_*` ở `queue.rs`, `fisher_yates_reaches_every_order_about_evenly` |
| S5 | test | `lyrics_use_id3v23_and_record_the_stamp`, `c3_failed_save_keeps_the_dialog_open_with_the_typed_text` |
| S6 | test | `a_write_whose_stamp_is_stored_is_ignored_in_every_batch` |
| S7 | test | `apply_scanned_track_writes_every_field_and_seeds_a_new_album_cover`, `rescan_picks_up_lyrics_edited_outside_the_app` |
| S8 | test | `saving_one_song_does_not_change_a_group_cover_that_exists`, `custom_album_cover_is_never_seeded_over` |
| S9 | test | `failed_or_panicking_transaction_rolls_back_and_the_writer_lives_on`, `writes_go_in_batches_with_progress_and_end_with_synced` |
| S10 | test | 17 test `covers.rs`, `large_cover_costs_about_one_decode_and_is_released`, `image_bombs_are_refused_before_decoding` |
| S11 | chỉ đọc mã | `Route::Category(CategoryRef)` không chở ảnh |
| S12 | test | `json_holds_references_only_with_camel_case_keys`, `saver_coalesces_writes_and_keeps_the_latest` |
| S13 | test | `cover_is_set_and_bad_images_are_reported`; menu thẻ "Đổi ảnh bìa" có ở `card_menu.rs` |
| S14 | chỉ đọc mã | `tags::read_track` đọc một lần |
| S15 | test | `migrated_and_new_libraries_have_the_same_schema` (schema có index) |
| S16 | test | `renders_only_visible_rows`; lưới, hàng đợi, hộp đoạn cũng dùng `VirtualList` / `VirtualGrid` (chỉ đọc mã) |
| S17 | test | `multi_value_frames_become_semicolons_instead_of_sticking_together`, `vorbis_multi_artist_is_joined` |
| S18 | test | `search_is_diacritic_free_and_never_matches_everything`, `stale_search_is_dropped_and_sequence_numbers_grow`, `fast_typing_keeps_only_the_last_search` |
| S19 | test | `album_orders_by_disc_then_track_folder_by_filename`, `playlist_keeps_insertion_order_after_removal` |
| S20 | test | `s20_*` ở `timeline.rs`, `scrub_seeks_only_on_release` |
| S21 | test | `quick_actions_all_run`, `cli_commands_do_real_work`, `help_keeps_the_palette_and_lists_commands` |
| S22 | test (Windows) / chưa kiểm (macOS Apple Event) | `extensions_match_the_association_list_in_any_case`, `file_urls_decode`, `initial_paths_come_first` |
| S23 | test | `preamp_keeps_whole_chain_at_or_below_unity`, `volume_and_mute_reach_the_engine`. Nghe thật: chưa kiểm |
| S24 | test | `write_mode_decides_whether_junk_frames_survive`, `renders_every_tab_and_ignores_a_second_save_while_saving` |
| S25 | test | `panic_in_decoder_becomes_an_error`, `panic_becomes_an_error`, `damaged_files_never_panic_out` |
| S26 | test | `a_link_cycle_ends_and_protects_what_is_behind_it`, `reimport_skips_unchanged_files_and_rereads_changed_ones`. OneDrive thật: chưa kiểm |
| M1 | test | `override_moves_data_and_cache_together`, `override_keeps_the_real_legacy_db_out_of_reach` (thêm ở task này: trước đó `GROOVE_DATA_DIR` vẫn chép DB cũ thật của người dùng) |
| M2 | test | `upsert_goes_through_path_key_and_keeps_the_id`, `duplicate_paths_merge_into_the_lowest_id` |
| M3 | test | `c5_escape_cancels_and_backdrop_does_not_close` |
| M4 | test | `open_dialog_blocks_global_keys`, `text_input_blocks_all_but_ctrl_k` |
| M5 | test | `unknown_command_is_reported`, `duplicate_key_is_reported_across_spellings` |
| M6 | test | `fixtures_are_readable_audio_of_about_one_second` |
| M7 | test | `choosing_custom_again_restores_the_hand_values` |
| M8 | chỉ đọc mã | log có mức; `println!` chỉ còn ở `dev.rs` cho `tools/run.ps1` |

## Số đo RAM

Đo 02/11/2026 bằng `tools/measure-hidden.ps1` (desktop Win32 ẩn, thư viện trống, không phát nhạc), máy Windows 10 của chủ
dự án. Exe release có `debug-assertions` (để `GROOVE_DATA_DIR` còn hiệu lực, không chạm dữ liệu thật), cùng LTO và tối
ưu như bản phát hành; bản phát hành thật chưa đo vì sẽ mở thư viện thật.

| Bản | Lúc | Working set | Private | Luồng |
| --- | --- | --- | --- | --- |
| release + debug assertions | nghỉ 10-30 s, My Music rỗng | 59,5 MB | 66,4 MB | 48 |
| release + debug assertions | đi qua Now Playing, bảng lệnh, Settings rồi về | 63,4-64,0 MB | 68,5-69,0 MB | 48 |
| debug | nghỉ | 76,4 MB | 69,8 MB | 48 |

Chưa đo: thư viện thật (1 002 bài), khi đang phát, trên màn thật có nền mờ, macOS, Linux. Lệnh:

```powershell
$env:CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS = "true"; cargo build --release -p groove-app
pwsh tools/measure-hidden.ps1 -Exe target/release/groove.exe -Seconds 30
```
