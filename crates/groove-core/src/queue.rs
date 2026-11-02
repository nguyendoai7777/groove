//! groove-gd1_12: hàng đợi, shuffle, repeat là hàm thuần (bảng 1.4, S4).
//!
//! Không biết gì về engine, thư viện hay đĩa: mỗi thao tác đổi ba danh sách rồi trả `QueueOutcome`, `app/player.rs` đổi kết
//! quả đó thành lệnh cho engine (N8). Hiển thị luôn là history + current + queue, và `next` luôn lấy đầu queue: thứ tự nhìn
//! thấy chính là thứ tự sẽ phát, nên Prev rồi Next về đúng bài cũ (S4).

use std::collections::HashSet;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

use crate::model::{RepeatMode, TrackRef};
use crate::session::Session;

/// Việc engine phải làm sau một thao tác trên hàng đợi. Trạng thái phát chỉ có một nguồn là engine, hàng đợi không giữ (N8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueOutcome {
    /// Nạp bài này và phát từ đầu.
    Play(TrackRef),
    /// Cùng bài đang chọn: giữ nguyên tiếng, đang dừng thì tiếp tục.
    Resume,
    /// Hết bài với repeat bật: phát lại bài hiện tại từ đầu.
    Replay,
    /// Prev khi history rỗng: tua về 0, giữ nguyên trạng thái phát.
    SeekToStart,
    /// Next ở cuối hàng đợi, repeat tắt: `pause` + `seek(0)` (N8).
    Stop,
    /// Không có bài nào để làm gì.
    Nothing,
}

/// Hàng đợi phát. Bất biến: mỗi bài xuất hiện một lần trong history + current + queue; `original` không trùng; shuffle và
/// repeat không cùng bật.
#[derive(Clone, Debug)]
pub struct Queue {
    current: Option<TrackRef>,
    original: Vec<TrackRef>,
    history: Vec<TrackRef>,
    queue: Vec<TrackRef>,
    shuffle: bool,
    repeat: RepeatMode,
    rng: SplitMix64,
}

impl Default for Queue {
    fn default() -> Queue {
        Queue::new()
    }
}

impl Queue {
    pub fn new() -> Queue {
        Queue::with_seed(entropy_seed())
    }

    /// Cùng hạt giống và cùng chuỗi thao tác cho cùng kết quả, để test được.
    pub fn with_seed(seed: u64) -> Queue {
        Queue {
            current: None,
            original: Vec::new(),
            history: Vec::new(),
            queue: Vec::new(),
            shuffle: false,
            repeat: RepeatMode::Off,
            rng: SplitMix64(seed),
        }
    }

    /// Dựng từ phiên đã lưu. Thứ tự queue giữ nguyên, kể cả khi đang shuffle: không xáo lại lúc mở app. Không tự phát (đặc tả
    /// 1.4), vị trí và việc nạp bài là của `app/player.rs`.
    pub fn from_session(session: &Session) -> Queue {
        let mut clean = session.clone();
        clean.sanitize();
        Queue {
            current: clean.current,
            original: clean.original,
            history: clean.history,
            queue: clean.queue,
            shuffle: clean.shuffle,
            repeat: clean.repeat,
            rng: SplitMix64(entropy_seed()),
        }
    }

    /// Chép trạng thái vào phiên, không đụng `position_sec`.
    pub fn write_to(&self, session: &mut Session) {
        session.current = self.current.clone();
        session.original = self.original.clone();
        session.history = self.history.clone();
        session.queue = self.queue.clone();
        session.shuffle = self.shuffle;
        session.repeat = self.repeat;
    }

    pub fn current(&self) -> Option<&TrackRef> {
        self.current.as_ref()
    }

    pub fn original(&self) -> &[TrackRef] {
        &self.original
    }

    pub fn history(&self) -> &[TrackRef] {
        &self.history
    }

    pub fn queue(&self) -> &[TrackRef] {
        &self.queue
    }

    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    /// Danh sách hiển thị theo đúng thứ tự: đã phát, đang phát, sắp phát.
    pub fn visible(&self) -> impl Iterator<Item = &TrackRef> {
        self.history.iter().chain(self.current.iter()).chain(self.queue.iter())
    }

    /// Vị trí bài đang phát trong `visible`.
    pub fn current_index(&self) -> Option<usize> {
        self.current.as_ref().map(|_| self.history.len())
    }

    /// Số bài trong `visible`, cho tiêu đề `(<n> tracks)` của bảng hàng đợi.
    pub fn visible_len(&self) -> usize {
        self.history.len() + usize::from(self.current.is_some()) + self.queue.len()
    }

    /// Phát kèm danh sách mới: original = danh sách, history rỗng, queue = phần sau bài (shuffle: mọi bài còn lại, xáo một
    /// lần). Trùng bài đang phát thì vẫn thay hàng đợi, chỉ giữ nguyên tiếng (S2). Danh sách rỗng coi như không kèm danh sách.
    pub fn play_list(&mut self, track: TrackRef, list: Vec<TrackRef>) -> QueueOutcome {
        if list.is_empty() {
            return self.play_track(track);
        }
        self.original = unique(list);
        self.history.clear();
        self.queue = match self.original.iter().position(|t| *t == track) {
            Some(ix) if !self.shuffle => self.original[ix + 1..].to_vec(),
            _ => self.original.iter().filter(|t| **t != track).cloned().collect(),
        };
        if self.shuffle {
            self.rng.shuffle(&mut self.queue);
        }
        self.select(track)
    }

    /// Phát không kèm danh sách (bấm trong hàng đợi, chọn bài từ bảng lệnh): bài cũ vào history, bài được chọn ra khỏi history
    /// và queue, bài khác giữ nguyên chỗ. Trùng bài đang phát thì chỉ tiếp tục.
    pub fn play_track(&mut self, track: TrackRef) -> QueueOutcome {
        if self.current.as_ref() == Some(&track) {
            return QueueOutcome::Resume;
        }
        if let Some(old) = self.current.take() {
            self.history.push(old);
        }
        self.history.retain(|t| *t != track);
        self.queue.retain(|t| *t != track);
        self.select(track)
    }

    /// Thêm vào cuối hàng đợi (`/queue`, mở nhiều tệp). Bài đã có trong history thì chuyển sang queue để nghe lại, bài đang phát
    /// thì bỏ qua. Không đụng `original`, nên vòng lặp repeat không gồm các bài này. Chưa có bài nào thì phát bài đầu.
    pub fn append(&mut self, tracks: impl IntoIterator<Item = TrackRef>) -> QueueOutcome {
        // Dùng tập hợp: mở cả nghìn tệp ngoài mà so từng cặp thì mỗi lần so lại chuẩn hoá đường dẫn.
        let mut taken: HashSet<TrackRef> = self.queue.iter().chain(self.current.iter()).cloned().collect();
        let added: Vec<TrackRef> = tracks.into_iter().filter(|track| taken.insert(track.clone())).collect();
        let added_set: HashSet<&TrackRef> = added.iter().collect();
        self.history.retain(|track| !added_set.contains(track));
        self.queue.extend(added);
        if self.current.is_none() && !self.queue.is_empty() {
            return self.select_next();
        }
        QueueOutcome::Nothing
    }

    /// Next. Queue còn: lấy đầu queue, bài cũ vào history. Queue hết: repeat bật thì quay vòng từ đầu original, tắt thì dừng.
    pub fn next_track(&mut self) -> QueueOutcome {
        if !self.queue.is_empty() {
            if let Some(old) = self.current.take() {
                self.history.push(old);
            }
            return self.select_next();
        }
        if self.repeat == RepeatMode::One {
            return self.wrap_around();
        }
        if self.current.is_some() { QueueOutcome::Stop } else { QueueOutcome::Nothing }
    }

    /// Prev. History rỗng thì tua về 0; có thì lấy cuối history, bài hiện tại vào đầu queue. Không có luật "quá 3 giây thì về đầu
    /// bài" (đặc tả 1.4).
    pub fn prev_track(&mut self) -> QueueOutcome {
        let Some(track) = self.history.pop() else {
            return if self.current.is_some() { QueueOutcome::SeekToStart } else { QueueOutcome::Nothing };
        };
        if let Some(old) = self.current.take() {
            self.queue.insert(0, old);
        }
        self.select(track)
    }

    /// Bài hết tự nhiên: repeat bật thì phát lại đúng bài này, tắt thì như Next.
    pub fn track_ended(&mut self) -> QueueOutcome {
        if self.repeat == RepeatMode::One && self.current.is_some() {
            return QueueOutcome::Replay;
        }
        self.next_track()
    }

    /// Phím `play_random`: bật shuffle, phát ngẫu nhiên một bài khác trong danh sách hiển thị (đặc tả 1.10). Không còn bài khác
    /// thì tiếp tục bài đang chọn.
    pub fn play_random(&mut self) -> QueueOutcome {
        self.set_shuffle(true);
        let others = self.history.len() + self.queue.len();
        if others == 0 {
            return if self.current.is_some() { QueueOutcome::Resume } else { QueueOutcome::Nothing };
        }
        let ix = self.rng.below(others);
        let track = match ix.checked_sub(self.history.len()) {
            Some(in_queue) => self.queue[in_queue].clone(),
            None => self.history[ix].clone(),
        };
        self.play_track(track)
    }

    pub fn toggle_shuffle(&mut self) {
        self.set_shuffle(!self.shuffle);
    }

    /// Bật: tắt repeat; queue = các bài chưa phát của original cộng bài đang chờ ngoài original, xáo cố định một lần. Tắt: khôi
    /// phục theo original (S4). Đặt lại đúng giá trị đang có thì không làm gì, không xáo lại.
    pub fn set_shuffle(&mut self, on: bool) {
        if self.shuffle == on {
            return;
        }
        self.shuffle = on;
        if on {
            self.repeat = RepeatMode::Off;
            self.queue = self.unplayed_in_original_order(0);
            self.rng.shuffle(&mut self.queue);
        } else {
            let after_current = self.original_index_of_current().map_or(0, |ix| ix + 1);
            self.queue = self.unplayed_in_original_order(after_current);
        }
    }

    pub fn toggle_repeat(&mut self) {
        self.set_repeat(match self.repeat {
            RepeatMode::Off => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        });
    }

    /// Bật repeat thì tắt shuffle, kèm khôi phục hàng đợi như `set_shuffle(false)`.
    pub fn set_repeat(&mut self, repeat: RepeatMode) {
        self.repeat = repeat;
        if repeat != RepeatMode::Off {
            self.set_shuffle(false);
        }
    }

    /// Bỏ bài không còn trong thư viện (N5). Trả `true` nếu mất bài đang phát: khi đó không còn bài nào được chọn, hàng đợi
    /// giữ nguyên, phát gì tiếp là việc của `app/player.rs`.
    pub fn retain_tracks(&mut self, mut keep: impl FnMut(&TrackRef) -> bool) -> bool {
        self.original.retain(&mut keep);
        self.history.retain(&mut keep);
        self.queue.retain(&mut keep);
        let lost_current = self.current.as_ref().is_some_and(|track| !keep(track));
        if lost_current {
            self.current = None;
        }
        lost_current
    }

    /// Xoá sạch hàng đợi và bài đang chọn, giữ shuffle và repeat.
    pub fn clear(&mut self) {
        self.current = None;
        self.original.clear();
        self.history.clear();
        self.queue.clear();
    }

    fn select(&mut self, track: TrackRef) -> QueueOutcome {
        let same = self.current.as_ref() == Some(&track);
        self.current = Some(track.clone());
        if same { QueueOutcome::Resume } else { QueueOutcome::Play(track) }
    }

    /// Đặt đầu queue làm bài hiện tại. Bên gọi đã xử lý bài cũ và biết queue còn bài.
    fn select_next(&mut self) -> QueueOutcome {
        let track = self.queue.remove(0);
        self.current = Some(track.clone());
        QueueOutcome::Play(track)
    }

    /// History xoá, nạp lại từ đầu original. Phát toàn tệp ngoài (original rỗng, N6) thì vòng qua các bài đã nghe, bài đang phát
    /// cuối cùng; bản cũ thoát sớm ở đây làm UI kẹt.
    fn wrap_around(&mut self) -> QueueOutcome {
        let source = if self.original.is_empty() {
            let mut played = std::mem::take(&mut self.history);
            played.extend(self.current.take());
            played
        } else {
            self.original.clone()
        };
        self.history.clear();
        self.queue = source;
        if self.queue.is_empty() {
            return QueueOutcome::Nothing;
        }
        self.current = None;
        self.select_next()
    }

    fn original_index_of_current(&self) -> Option<usize> {
        let current = self.current.as_ref()?;
        self.original.iter().position(|t| t == current)
    }

    /// Bài chưa phát của original, bắt đầu từ `start` rồi vòng qua phần trước nó, cuối cùng là bài đang chờ ngoài original. Bài
    /// sau vị trí hiện tại phát trước như bản cũ, bài trước vị trí đó không bị mất (S4).
    fn unplayed_in_original_order(&self, start: usize) -> Vec<TrackRef> {
        let played: HashSet<&TrackRef> = self.history.iter().chain(self.current.iter()).collect();
        let in_original: HashSet<&TrackRef> = self.original.iter().collect();
        let (before, after) = self.original.split_at(start.min(self.original.len()));
        after
            .iter()
            .chain(before)
            .filter(|t| !played.contains(t))
            .chain(self.queue.iter().filter(|t| !in_original.contains(t)))
            .cloned()
            .collect()
    }
}

fn unique(list: Vec<TrackRef>) -> Vec<TrackRef> {
    let mut seen = HashSet::new();
    list.into_iter().filter(|track| seen.insert(track.clone())).collect()
}

/// Hạt giống không cần thêm crate: `RandomState` của std lấy từ nguồn ngẫu nhiên của OS.
fn entropy_seed() -> u64 {
    RandomState::new().build_hasher().finish()
}

/// Bộ sinh số nhỏ cho việc xáo bài, không cho mật mã.
#[derive(Clone, Debug)]
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Số trong `0..n`, `n > 0`. Độ lệch của phép nhân 128 bit không đáng kể với cỡ hàng đợi.
    fn below(&mut self, n: usize) -> usize {
        ((u128::from(self.next_u64()) * n as u128) >> 64) as usize
    }

    /// Fisher-Yates.
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn t(id: i64) -> TrackRef {
        TrackRef::Library(id)
    }

    fn lib(ids: &[i64]) -> Vec<TrackRef> {
        ids.iter().copied().map(t).collect()
    }

    fn sorted(tracks: &[TrackRef]) -> Vec<i64> {
        let mut ids: Vec<i64> = tracks
            .iter()
            .map(|track| match track {
                TrackRef::Library(id) => *id,
                TrackRef::External(_) => panic!("chỉ dùng cho bài thư viện"),
            })
            .collect();
        ids.sort_unstable();
        ids
    }

    fn playing(id: i64, list: &[i64]) -> Queue {
        let mut q = Queue::with_seed(7);
        q.play_list(t(id), lib(list));
        q
    }

    fn assert_invariants(q: &Queue) {
        let visible: Vec<&TrackRef> = q.visible().collect();
        let set: HashSet<&TrackRef> = visible.iter().copied().collect();
        assert_eq!(set.len(), visible.len(), "mỗi bài một lần: {q:?}");
        assert_eq!(q.visible_len(), visible.len());
        assert!(!(q.shuffle() && q.repeat() == RepeatMode::One), "shuffle và repeat không cùng bật: {q:?}");
    }

    // Bảng 1.4, từng dòng.

    #[test]
    fn play_with_list_takes_the_part_after_the_track() {
        let mut q = Queue::with_seed(1);
        assert_eq!(q.play_list(t(2), lib(&[1, 2, 3, 4])), QueueOutcome::Play(t(2)));
        assert_eq!(q.original(), lib(&[1, 2, 3, 4]));
        assert!(q.history().is_empty());
        assert_eq!(q.queue(), lib(&[3, 4]));
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn play_with_list_under_shuffle_queues_every_other_track() {
        let mut q = Queue::with_seed(1);
        q.set_shuffle(true);
        q.play_list(t(2), lib(&[1, 2, 3, 4, 5]));
        assert_eq!(sorted(q.queue()), [1, 3, 4, 5]);
        assert!(q.history().is_empty());
        assert_eq!(q.original(), lib(&[1, 2, 3, 4, 5]), "original giữ thứ tự nguồn");
    }

    #[test]
    fn play_with_list_replaces_the_old_history() {
        let mut q = playing(1, &[1, 2, 3]);
        q.next_track();
        q.play_list(t(8), lib(&[8, 9]));
        assert!(q.history().is_empty());
        assert_eq!(q.queue(), lib(&[9]));
    }

    #[test]
    fn play_with_list_not_containing_the_track_queues_the_whole_list() {
        let mut q = Queue::with_seed(1);
        q.play_list(t(9), lib(&[1, 2]));
        assert_eq!(q.current(), Some(&t(9)));
        assert_eq!(q.queue(), lib(&[1, 2]));
    }

    #[test]
    fn duplicates_in_a_new_list_appear_once() {
        let mut q = Queue::with_seed(1);
        q.play_list(t(1), lib(&[1, 2, 2, 1, 3]));
        assert_eq!(q.original(), lib(&[1, 2, 3]));
        assert_eq!(q.queue(), lib(&[2, 3]));
        assert_invariants(&q);
    }

    #[test]
    fn empty_list_counts_as_no_list() {
        let mut q = playing(1, &[1, 2, 3]);
        assert_eq!(q.play_list(t(3), Vec::new()), QueueOutcome::Play(t(3)));
        assert_eq!(q.original(), lib(&[1, 2, 3]));
        assert_eq!(q.history(), lib(&[1]));
        assert_eq!(q.queue(), lib(&[2]));
    }

    #[test]
    fn play_without_list_moves_old_track_to_history_and_keeps_the_others_in_place() {
        let mut q = playing(1, &[1, 2, 3, 4, 5]);
        q.next_track(); // history [1], current 2, queue [3, 4, 5]
        assert_eq!(q.play_track(t(4)), QueueOutcome::Play(t(4)));
        assert_eq!(q.history(), lib(&[1, 2]));
        assert_eq!(q.queue(), lib(&[3, 5]));
        assert_eq!(q.play_track(t(1)), QueueOutcome::Play(t(1)));
        assert_eq!(q.history(), lib(&[2, 4]));
        assert_eq!(q.queue(), lib(&[3, 5]));
    }

    #[test]
    fn playing_the_current_track_only_resumes() {
        let mut q = playing(1, &[1, 2, 3]);
        assert_eq!(q.play_track(t(1)), QueueOutcome::Resume);
        assert!(q.history().is_empty());
        assert_eq!(q.queue(), lib(&[2, 3]));
    }

    #[test]
    fn s2_playing_the_current_track_with_a_new_list_still_replaces_the_queue() {
        // S2: bản cũ về sớm, Play all ở album có bài đang phát đứng đầu không đổi hàng đợi.
        let mut q = playing(1, &[7, 1, 8]);
        assert_eq!(q.play_list(t(1), lib(&[1, 2, 3])), QueueOutcome::Resume);
        assert_eq!(q.original(), lib(&[1, 2, 3]));
        assert_eq!(q.queue(), lib(&[2, 3]));
    }

    #[test]
    fn next_takes_the_head_of_the_queue_and_moves_old_track_to_history() {
        let mut q = playing(1, &[1, 2, 3]);
        assert_eq!(q.next_track(), QueueOutcome::Play(t(2)));
        assert_eq!(q.history(), lib(&[1]));
        assert_eq!(q.queue(), lib(&[3]));
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn next_under_shuffle_plays_exactly_the_order_shown() {
        // S4: bản cũ bốc ngẫu nhiên từng bước, hàng đợi hiển thị không phải thứ tự sẽ phát.
        let mut q = playing(1, &[1, 2, 3, 4, 5, 6, 7, 8]);
        q.set_shuffle(true);
        let shown = q.queue().to_vec();
        for track in shown {
            assert_eq!(q.next_track(), QueueOutcome::Play(track));
        }
        assert!(q.queue().is_empty());
    }

    #[test]
    fn next_at_the_end_with_repeat_reloads_from_the_start_of_original() {
        let mut q = playing(3, &[1, 2, 3]);
        q.set_repeat(RepeatMode::One);
        assert_eq!(q.next_track(), QueueOutcome::Play(t(1)));
        assert!(q.history().is_empty());
        assert_eq!(q.queue(), lib(&[2, 3]));
        assert_invariants(&q);
    }

    #[test]
    fn next_at_the_end_with_repeat_on_a_single_track_replays_it() {
        let mut q = playing(1, &[1]);
        q.set_repeat(RepeatMode::One);
        assert_eq!(q.next_track(), QueueOutcome::Play(t(1)));
        assert_eq!(q.current(), Some(&t(1)));
        assert!(q.queue().is_empty());
    }

    #[test]
    fn n8_next_at_the_end_without_repeat_stops_and_keeps_the_track() {
        let mut q = playing(2, &[1, 2]);
        assert_eq!(q.next_track(), QueueOutcome::Stop);
        assert_eq!(q.current(), Some(&t(2)));
        assert!(q.history().is_empty());
    }

    #[test]
    fn prev_with_empty_history_seeks_to_start() {
        let mut q = playing(1, &[1, 2]);
        assert_eq!(q.prev_track(), QueueOutcome::SeekToStart);
        assert_eq!(q.current(), Some(&t(1)));
        assert_eq!(q.queue(), lib(&[2]));
    }

    #[test]
    fn prev_takes_the_end_of_history_and_puts_current_at_the_head_of_the_queue() {
        let mut q = playing(1, &[1, 2, 3]);
        q.next_track();
        assert_eq!(q.prev_track(), QueueOutcome::Play(t(1)));
        assert!(q.history().is_empty());
        assert_eq!(q.queue(), lib(&[2, 3]));
    }

    #[test]
    fn s4_prev_then_next_returns_to_the_same_track_under_shuffle() {
        let mut q = playing(1, &[1, 2, 3, 4, 5, 6]);
        q.set_shuffle(true);
        q.next_track();
        let QueueOutcome::Play(second) = q.next_track() else { panic!("queue còn bài") };
        q.prev_track();
        assert_eq!(q.next_track(), QueueOutcome::Play(second));
    }

    #[test]
    fn ended_with_repeat_replays_the_same_track_without_touching_the_lists() {
        let mut q = playing(1, &[1, 2]);
        q.set_repeat(RepeatMode::One);
        assert_eq!(q.track_ended(), QueueOutcome::Replay);
        assert_eq!(q.current(), Some(&t(1)));
        assert_eq!(q.queue(), lib(&[2]));
    }

    #[test]
    fn ended_without_repeat_is_next() {
        let mut q = playing(1, &[1, 2]);
        assert_eq!(q.track_ended(), QueueOutcome::Play(t(2)));
        assert_eq!(q.track_ended(), QueueOutcome::Stop);
    }

    #[test]
    fn shuffle_on_turns_repeat_off_and_queues_all_unplayed_of_original() {
        let mut q = playing(3, &[1, 2, 3, 4, 5]);
        q.next_track(); // history [3], current 4, queue [5]
        q.set_repeat(RepeatMode::One);
        q.set_shuffle(true);
        assert_eq!(q.repeat(), RepeatMode::Off);
        assert_eq!(sorted(q.queue()), [1, 2, 5], "trừ bài đang phát và bài đã phát");
        assert_eq!(q.history(), lib(&[3]));
    }

    #[test]
    fn repeat_on_turns_shuffle_off_and_restores_the_queue() {
        let mut q = playing(2, &[1, 2, 3, 4]);
        q.set_shuffle(true);
        q.set_repeat(RepeatMode::One);
        assert!(!q.shuffle());
        assert_eq!(q.queue(), lib(&[3, 4, 1]));
    }

    #[test]
    fn repeat_off_leaves_shuffle_alone() {
        let mut q = playing(2, &[1, 2, 3, 4]);
        q.set_shuffle(true);
        let shown = q.queue().to_vec();
        q.set_repeat(RepeatMode::Off);
        assert!(q.shuffle());
        assert_eq!(q.queue(), shown);
    }

    // S4.

    #[test]
    fn s4_shuffle_keeps_tracks_waiting_outside_original() {
        // Bản cũ dựng lại từ original nên mất phần còn lại của hàng đợi.
        let ext = TrackRef::from_path("/m/ngoài.mp3");
        let mut q = playing(1, &[1, 2, 3]);
        q.append([ext.clone()]);
        q.set_shuffle(true);
        assert!(q.queue().contains(&ext));
        q.set_shuffle(false);
        assert_eq!(q.queue(), [t(2), t(3), ext]);
    }

    #[test]
    fn s4_shuffle_off_restores_unplayed_tracks_on_both_sides() {
        // Bản cũ chỉ lấy phần sau bài hiện tại, các bài phía trước chưa nghe bị mất.
        let mut q = playing(1, &[1, 2, 3, 4, 5, 6]);
        q.set_shuffle(true);
        // Nghe tới bài 4 theo đường ngắn nhất, các bài còn lại nằm cả hai phía của nó trong original.
        assert_eq!(q.play_track(t(4)), QueueOutcome::Play(t(4)));
        q.set_shuffle(false);
        assert_eq!(q.history(), lib(&[1]));
        assert_eq!(q.queue(), lib(&[5, 6, 2, 3]), "phía sau trước, rồi phía trước chưa nghe");
        assert_invariants(&q);
    }

    #[test]
    fn s4_shuffle_off_skips_tracks_already_played_after_the_current_position() {
        let mut q = playing(1, &[1, 2, 3, 4]);
        q.set_shuffle(true);
        q.play_track(t(4));
        q.play_track(t(2));
        q.set_shuffle(false);
        assert_eq!(q.history(), lib(&[1, 4]));
        assert_eq!(q.queue(), lib(&[3]));
    }

    #[test]
    fn shuffle_is_fixed_once_and_setting_it_again_does_not_reshuffle() {
        let mut q = playing(1, &(1..=30).collect::<Vec<_>>());
        q.set_shuffle(true);
        let shown = q.queue().to_vec();
        q.set_shuffle(true);
        assert_eq!(q.queue(), shown);
        assert_eq!(sorted(&shown), (2..=30).collect::<Vec<_>>());
        assert_ne!(shown, lib(&(2..=30).collect::<Vec<_>>()), "xáo thật");
    }

    #[test]
    fn fisher_yates_reaches_every_order_about_evenly() {
        let mut rng = SplitMix64(42);
        let mut counts: HashMap<[u8; 3], u32> = HashMap::new();
        for _ in 0..6000 {
            let mut items = [1u8, 2, 3];
            rng.shuffle(&mut items);
            *counts.entry(items).or_default() += 1;
        }
        assert_eq!(counts.len(), 6);
        assert!(counts.values().all(|n| (850..=1150).contains(n)), "{counts:?}");
    }

    // N5, N6.

    #[test]
    fn n6_second_external_file_plays_and_the_end_does_not_stick() {
        // Bản cũ so `id = -1` nên tệp thứ hai không phát, Next thoát sớm khi original rỗng.
        let a = TrackRef::from_path("/m/a.mp3");
        let b = TrackRef::from_path("/m/b.mp3");
        let mut q = Queue::with_seed(1);
        assert_eq!(q.play_track(a.clone()), QueueOutcome::Play(a.clone()));
        assert_eq!(q.play_track(b.clone()), QueueOutcome::Play(b.clone()));
        assert_eq!(q.history(), std::slice::from_ref(&a));
        assert_eq!(q.track_ended(), QueueOutcome::Stop);
        q.set_repeat(RepeatMode::One);
        assert_eq!(q.next_track(), QueueOutcome::Play(a.clone()), "vòng qua các bài đã nghe");
        assert_eq!(q.queue(), [b]);
        assert_invariants(&q);
    }

    #[test]
    fn path_spellings_of_one_file_are_the_current_track() {
        let mut q = Queue::with_seed(1);
        q.play_track(TrackRef::from_path(r"C:\Music\a.mp3"));
        assert_eq!(q.play_track(TrackRef::from_path("C:/Music/a.mp3")), QueueOutcome::Resume);
    }

    #[test]
    fn append_goes_to_the_end_and_starts_when_nothing_is_selected() {
        let mut q = Queue::with_seed(1);
        assert_eq!(q.append(lib(&[1, 2, 2])), QueueOutcome::Play(t(1)));
        assert_eq!(q.queue(), lib(&[2]));
        assert_eq!(q.append(lib(&[1, 2, 3])), QueueOutcome::Nothing);
        assert_eq!(q.queue(), lib(&[2, 3]));
        q.next_track();
        assert_eq!(q.append(lib(&[1])), QueueOutcome::Nothing);
        assert!(q.history().is_empty(), "bài đã nghe chuyển sang queue, không trùng");
        assert_eq!(q.queue(), lib(&[3, 1]));
        assert_invariants(&q);
    }

    #[test]
    fn append_nothing_on_an_empty_queue_is_nothing() {
        let mut q = Queue::with_seed(1);
        assert_eq!(q.append(Vec::new()), QueueOutcome::Nothing);
        assert_eq!(q.current(), None);
    }

    #[test]
    fn n5_dropping_dead_tracks_keeps_the_rest() {
        let mut q = playing(1, &[1, 2, 3, 4]);
        q.next_track();
        assert!(!q.retain_tracks(|track| *track != t(3) && *track != t(1)));
        assert_eq!(q.original(), lib(&[2, 4]));
        assert!(q.history().is_empty());
        assert_eq!(q.queue(), lib(&[4]));
        assert_eq!(q.current(), Some(&t(2)));
    }

    #[test]
    fn n5_losing_the_current_track_is_reported_and_next_still_works() {
        let mut q = playing(1, &[1, 2, 3]);
        assert!(q.retain_tracks(|track| *track != t(1)));
        assert_eq!(q.current(), None);
        assert_eq!(q.current_index(), None);
        assert_eq!(q.next_track(), QueueOutcome::Play(t(2)));
        assert!(q.history().is_empty(), "không có bài cũ để đưa vào history");
    }

    // Trạng thái rỗng.

    #[test]
    fn empty_queue_has_nothing_to_do() {
        let mut q = Queue::with_seed(1);
        assert_eq!(q.next_track(), QueueOutcome::Nothing);
        assert_eq!(q.prev_track(), QueueOutcome::Nothing);
        assert_eq!(q.track_ended(), QueueOutcome::Nothing);
        assert_eq!(q.play_random(), QueueOutcome::Nothing);
        q.set_repeat(RepeatMode::One);
        assert_eq!(q.next_track(), QueueOutcome::Nothing);
        assert_eq!(q.track_ended(), QueueOutcome::Nothing);
        q.set_shuffle(true);
        assert_eq!(q.visible_len(), 0);
    }

    #[test]
    fn prev_without_current_still_steps_back() {
        let mut q = playing(1, &[1, 2]);
        q.next_track();
        q.retain_tracks(|track| *track != t(2));
        assert_eq!(q.prev_track(), QueueOutcome::Play(t(1)));
        assert!(q.queue().is_empty());
    }

    #[test]
    fn play_random_turns_shuffle_on_and_picks_another_track() {
        let mut q = playing(1, &[1, 2, 3, 4]);
        q.next_track();
        let QueueOutcome::Play(track) = q.play_random() else { panic!("còn bài khác") };
        assert!(q.shuffle());
        assert_ne!(track, t(2));
        assert_eq!(q.current(), Some(&track));
        assert_invariants(&q);

        let mut alone = playing(1, &[1]);
        assert_eq!(alone.play_random(), QueueOutcome::Resume);
    }

    #[test]
    fn clear_forgets_tracks_but_not_the_modes() {
        let mut q = playing(1, &[1, 2]);
        q.set_repeat(RepeatMode::One);
        q.clear();
        assert_eq!(q.visible_len(), 0);
        assert!(q.original().is_empty());
        assert_eq!(q.repeat(), RepeatMode::One);
    }

    // Khớp với phiên.

    #[test]
    fn session_round_trip_keeps_the_exact_order() {
        let mut q = playing(1, &[1, 2, 3, 4, 5, 6]);
        q.set_shuffle(true);
        q.next_track();
        let mut session = Session { position_sec: 12.5, ..Session::default() };
        q.write_to(&mut session);
        assert_eq!(session.position_sec, 12.5, "vị trí không phải của hàng đợi");

        let back = Queue::from_session(&session);
        assert_eq!(back.current(), q.current());
        assert_eq!(back.original(), q.original());
        assert_eq!(back.history(), q.history());
        assert_eq!(back.queue(), q.queue(), "mở lại app không xáo lại");
        assert_eq!(back.shuffle(), q.shuffle());
        assert_eq!(back.repeat(), q.repeat());
    }

    #[test]
    fn broken_session_still_gives_a_valid_queue() {
        let session = Session {
            current: Some(t(1)),
            original: lib(&[1, 1, 2]),
            history: lib(&[1, 2]),
            queue: lib(&[1, 2, 3, 3]),
            shuffle: true,
            repeat: RepeatMode::One,
            ..Session::default()
        };
        let q = Queue::from_session(&session);
        assert_eq!(q.original(), lib(&[1, 2]));
        assert_eq!(q.queue(), lib(&[2, 3]));
        assert!(q.history().is_empty());
        assert_invariants(&q);
    }

    #[test]
    fn invariants_hold_under_any_sequence_of_actions() {
        let mut dice = SplitMix64(2026);
        for seed in 0..50 {
            let mut q = Queue::with_seed(seed);
            for _ in 0..300 {
                let id = dice.below(12) as i64;
                match dice.below(11) {
                    0 => drop(q.play_list(t(id), lib(&[1, 2, 3, 4, 5, 6, 7, 8]))),
                    1 => drop(q.play_track(t(id))),
                    2 | 3 => drop(q.next_track()),
                    4 => drop(q.prev_track()),
                    5 => drop(q.track_ended()),
                    6 => q.toggle_shuffle(),
                    7 => q.toggle_repeat(),
                    8 => drop(q.append([t(id)])),
                    9 => drop(q.play_random()),
                    _ => {
                        q.retain_tracks(|track| *track != t(id));
                    }
                }
                assert_invariants(&q);
            }
        }
    }
}
