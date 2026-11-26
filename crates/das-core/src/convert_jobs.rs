//! Conversion batches in the list: adding, running (apart from downloads, so they never take a
//! download slot or touch the speed limits), progress, pause, start over and remove.

use super::*;
use crate::convert::{self, ConvertEnd, ConvertFile, ConvertJob, ConvertSpec, FileOutcome, FileState, Preset};

/// Convert files and folders the user owns.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddConvertRequest {
    pub paths: Vec<String>,
    pub preset: Preset,
    /// A folder for every result (None = next to the originals, in a folder named after the preset).
    #[serde(default)]
    pub out_dir: Option<String>,
}

/// One file of a planned batch, for the convert sheet.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedFile {
    pub path: String,
    pub name: String,
    pub size: u64,
    /// "FLAC 24-bit 96 kHz"; None when ffprobe can't read it.
    pub format: Option<String>,
    pub verdict: Option<convert::Verdict>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertPlan {
    pub files: Vec<PlannedFile>,
    /// Where the results go (the first file's folder, for the sheet).
    pub out_dir: Option<String>,
    pub keeps_cover: bool,
    pub ffmpeg_ready: bool,
}

use serde::{Deserialize, Serialize};

/// How many ffmpeg processes one batch runs at once.
fn parallel() -> usize {
    std::thread::available_parallelism().map(|n| n.get() / 2).unwrap_or(2).clamp(1, 4)
}

impl Manager {
    /// What converting these files to `preset` would do (probes each file).
    pub async fn convert_plan(&self, paths: Vec<String>, preset: Preset, out_dir: Option<String>) -> ConvertPlan {
        let tools = self.inner.tools.clone();
        let ffmpeg_ready = tools.ffmpeg_ready();
        let inputs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        let found = tokio::task::spawn_blocking(move || convert::expand(&inputs)).await.unwrap_or_default();
        let mut files = Vec::with_capacity(found.len());
        for (p, _) in found.iter().take(2000) {
            let probe = if ffmpeg_ready { convert::probe(&tools.ffprobe(), p).await } else { None };
            files.push(PlannedFile {
                path: p.to_string_lossy().into_owned(),
                name: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                size: std::fs::metadata(p).map(|m| m.len()).unwrap_or(0),
                format: probe.as_ref().map(|x| x.describe()),
                verdict: probe.as_ref().map(|x| convert::verdict(x, preset)),
            });
        }
        let first = found.first();
        ConvertPlan {
            files,
            out_dir: first.map(|(p, root)| {
                convert::output_dir(p, root.as_deref(), out_dir.as_deref().map(Path::new), preset).to_string_lossy().into_owned()
            }),
            keeps_cover: preset.keeps_cover(),
            ffmpeg_ready,
        }
    }

    /// Add a conversion batch to the list and start it.
    pub async fn add_convert(&self, req: AddConvertRequest) -> Result<ItemView, String> {
        if !self.inner.tools.ffmpeg_ready() {
            return Err("Converting needs ffmpeg: get the media tools first".into());
        }
        let inputs: Vec<PathBuf> = req.paths.iter().map(PathBuf::from).collect();
        let found = tokio::task::spawn_blocking(move || convert::expand(&inputs)).await.unwrap_or_default();
        if found.is_empty() {
            return Err("No audio files there".into());
        }
        let ffprobe = self.inner.tools.ffprobe();
        let mut files = Vec::with_capacity(found.len());
        for (p, root) in &found {
            let probe = convert::probe(&ffprobe, p).await;
            files.push(ConvertFile {
                input: p.to_string_lossy().into_owned(),
                root: root.as_ref().map(|r| r.to_string_lossy().into_owned()),
                size: std::fs::metadata(p).map(|m| m.len()).unwrap_or(0),
                duration: probe.as_ref().map(|x| x.duration).unwrap_or(0.0),
                state: FileState::Pending,
                output: None,
                note: None,
            });
        }
        let (first, first_root) = &found[0];
        let out = convert::output_dir(first, first_root.as_deref(), req.out_dir.as_deref().map(Path::new), req.preset);
        let what = match (first_root, found.len()) {
            (Some(r), _) => r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            (None, 1) => first.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            (None, n) => format!("{} and {} more", first.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), n - 1),
        };
        let spec = ConvertSpec { preset: req.preset, out_dir: req.out_dir.clone(), files };
        let view = {
            let mut st = self.inner.state.lock();
            let position = st.next_position;
            st.next_position += 1;
            let item = Item {
                id: format!("{:x}{:04x}", now_ms(), fastrand::u16(..)),
                url: String::new(),
                file_name: format!("{what} \u{2192} {}", req.preset.label()),
                save_dir: out.to_string_lossy().into_owned(),
                file_path: None,
                size: Some(spec.files.iter().map(|f| f.size).sum()),
                downloaded: 0,
                status: Status::Queued,
                category: "music".into(),
                added_at: now_ms(),
                finished_at: None,
                error: None,
                connections: 0,
                resumable: true,
                headers: vec![],
                resume: None,
                position,
                name_fixed: true,
                allow_html: false,
                kind: Kind::Convert,
                video: None,
                torrent: None,
                convert: Some(spec),
            };
            let _ = self.inner.db.lock().save_item(&item);
            let view = ItemView::from(&item);
            st.items.insert(item.id.clone(), item);
            view
        };
        (self.inner.emit)(CoreEvent::Upsert { item: view.clone() });
        self.inner.schedule();
        Ok(view)
    }
}

impl Inner {
    /// Start the next queued batch (one at a time; its files run in parallel).
    pub(super) fn schedule_converts(self: &Arc<Self>) {
        if !self.tools.ffmpeg_ready() || self.quitting.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let (id, spec) = {
            let mut st = self.state.lock();
            if !st.converting.is_empty() {
                return;
            }
            let next = st
                .items
                .values()
                .filter(|i| i.kind == Kind::Convert && i.status == Status::Queued)
                .min_by_key(|i| i.position)
                .map(|i| i.id.clone());
            let Some(id) = next else { return };
            let item = st.items.get_mut(&id).unwrap();
            let Some(spec) = item.convert.clone() else { return };
            item.status = Status::Downloading;
            item.error = None;
            let view = ItemView::from(&*item);
            let _ = self.db.lock().save_item(item);
            drop(st);
            (self.emit)(CoreEvent::Upsert { item: view });
            (id, spec)
        };
        let me = self.clone();
        let id2 = id.clone();
        let on_file: Arc<dyn Fn(FileOutcome) + Send + Sync> = Arc::new(move |o| me.convert_file_done(&id2, o));
        let job = ConvertJob::start(self.tools.ffmpeg(), self.tools.ffprobe(), spec, parallel(), on_file);
        self.state.lock().converting.insert(id.clone(), job.clone());
        let me = self.clone();
        tokio::spawn(async move {
            let end = job.wait().await;
            me.finished_convert(&id, end);
        });
    }

    /// A file ended: remember it at once (its result is ours to delete later).
    fn convert_file_done(&self, id: &str, o: FileOutcome) {
        let item = {
            let mut st = self.state.lock();
            let Some(item) = st.items.get_mut(id) else { return };
            let Some(spec) = item.convert.as_mut() else { return };
            let Some(f) = spec.files.get_mut(o.index) else { return };
            f.state = o.state;
            f.output = o.output.map(|p| p.to_string_lossy().into_owned());
            f.note = o.note;
            item.downloaded = done_bytes(item.size, spec, 0.0);
            item.clone()
        };
        self.persist_and_emit(&[item]);
    }

    fn finished_convert(self: &Arc<Self>, id: &str, end: ConvertEnd) {
        if self.quitting.load(std::sync::atomic::Ordering::SeqCst) {
            return; // stays Downloading in the database; picked up again at the next start
        }
        let item = {
            let mut st = self.state.lock();
            st.converting.remove(id);
            let Some(item) = st.items.get_mut(id) else {
                drop(st);
                self.schedule();
                return;
            };
            let Some(spec) = item.convert.clone() else { return };
            convert::remove_temps(&spec);
            match end {
                ConvertEnd::Done => {
                    let failed = spec.files.iter().filter(|f| f.state == FileState::Failed).count();
                    let made: Vec<&String> = spec.files.iter().filter_map(|f| f.output.as_ref()).collect();
                    item.finished_at = Some(now_ms());
                    item.downloaded = item.size.unwrap_or(0);
                    item.file_path = match made.as_slice() {
                        [one] => Some((*one).clone()),
                        _ => Some(item.save_dir.clone()),
                    };
                    if made.is_empty() && failed > 0 {
                        item.status = Status::Failed;
                        item.error = spec.files.iter().find_map(|f| f.note.clone()).or(Some("ffmpeg couldn't convert these files".into()));
                    } else {
                        item.status = Status::Done;
                        item.error = None;
                    }
                }
                ConvertEnd::Paused => item.status = Status::Paused,
                ConvertEnd::Cancelled => {}
            }
            item.clone()
        };
        let done = item.status == Status::Done;
        self.persist_and_emit(&[item.clone()]);
        if done {
            (self.emit)(CoreEvent::Completed { item: ItemView::from(&item) });
        }
        self.schedule();
    }

    /// Live progress for the running batch.
    pub(super) fn convert_progress(&self, items: &mut HashMap<String, Item>, converting: &HashMap<String, ConvertJob>, out: &mut Vec<Progress>) {
        for (id, job) in converting {
            let Some(item) = items.get_mut(id) else { continue };
            let Some(spec) = item.convert.as_ref() else { continue };
            item.downloaded = done_bytes(item.size, spec, job.live_seconds());
            out.push(Progress {
                id: id.clone(),
                downloaded: item.downloaded,
                size: item.size,
                speed: 0,
                eta: None,
                active_connections: 0,
                gaps: vec![],
                heads: vec![],
                parts: vec![],
                parts_total: 0,
                file_name: item.file_name.clone(),
                phase: Some(crate::video::VideoPhase::Converting),
                torrent: None,
            });
        }
    }

    /// Pause a running batch; true if one was running.
    pub(super) fn pause_convert(&self, st: &State, id: &str) -> bool {
        match st.converting.get(id) {
            Some(job) => {
                job.pause();
                true
            }
            None => false,
        }
    }

    /// Start over: delete only the results this batch made, and convert everything again.
    pub(super) fn restart_convert(item: &mut Item) {
        let Some(spec) = item.convert.as_mut() else { return };
        for f in spec.files.iter_mut() {
            if let Some(o) = f.output.take() {
                let _ = std::fs::remove_file(o);
            }
            f.state = FileState::Pending;
            f.note = None;
        }
        convert::remove_temps(spec);
    }

    /// Remove a batch: stop it, clear its temporary files, and (only if asked) delete the
    /// results it made. The originals are never touched.
    pub(super) fn remove_convert(&self, job: Option<ConvertJob>, item: &Item, delete_files: bool) {
        let Some(spec) = item.convert.clone() else { return };
        let outputs: Vec<String> = if delete_files { spec.files.iter().filter_map(|f| f.output.clone()).collect() } else { vec![] };
        let cleanup = move || {
            convert::remove_temps(&spec);
            for o in &outputs {
                // Never an input: a result path is always one DasIDM wrote.
                if !spec.files.iter().any(|f| &f.input == o) {
                    let _ = std::fs::remove_file(o);
                }
            }
        };
        match job {
            Some(j) => {
                j.cancel();
                tokio::spawn(async move {
                    let _ = j.wait().await;
                    cleanup();
                });
            }
            None => cleanup(),
        }
    }
}

/// Bytes "done" for the list: the inputs' total size, in proportion to the audio converted.
fn done_bytes(size: Option<u64>, spec: &ConvertSpec, live: f64) -> u64 {
    let size = size.unwrap_or(0);
    let (total, done) = spec.seconds();
    if total <= 0.0 {
        let n = spec.files.len().max(1);
        let finished = spec.files.iter().filter(|f| f.state != FileState::Pending).count();
        return size * finished as u64 / n as u64;
    }
    ((size as f64) * ((done + live) / total).clamp(0.0, 1.0)) as u64
}
