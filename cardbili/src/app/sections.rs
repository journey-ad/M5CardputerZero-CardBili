use super::*;
use slint::ComponentHandle as _;

#[derive(Clone, Default)]
pub(super) struct Sections {
    pub bvid: String,
    pub cid: i64,
    pub parts: Vec<model::VideoPart>,
    pub chapters: Vec<model::Chapter>,
    pub loading: bool,
    pub error: String,
    pub open: bool,
    pub selection: usize,
    pub first: usize,
    pub dirty: bool,
}

impl Sections {
    pub fn part_count(&self) -> usize {
        if self.parts.len() > 1 {
            self.parts.len()
        } else {
            0
        }
    }
    pub fn available(&self) -> bool {
        self.part_count() > 0 || !self.chapters.is_empty()
    }
}

impl App {
    pub(super) fn chapters_loaded(
        &mut self,
        cid: i64,
        result: Result<Vec<model::Chapter>, String>,
    ) {
        if self.sections.cid == cid {
            self.sections.loading = false;
            match result {
                Ok(chapters) => self.sections.chapters = chapters,
                Err(error) => self.sections.error = error,
            }
            self.sections.dirty = true;
        }
    }

    pub(super) fn setup_sections(&mut self, bvid: String, cid: i64, parts: Vec<model::VideoPart>) {
        if self.sections.cid == cid {
            return;
        }
        self.sections = Sections {
            bvid: bvid.clone(),
            cid,
            parts,
            loading: true,
            dirty: true,
            ..Default::default()
        };
        self.load_chapters();
    }

    pub(super) fn load_chapters(&mut self) {
        self.sections.loading = true;
        self.sections.error.clear();
        tasks::playback::chapters(
            self.sections.bvid.clone(),
            self.sections.cid,
            self.cookies.clone(),
            self.tx.clone(),
        );
    }

    pub(super) fn sections_key(&mut self, key: &str) {
        let count = self.sections.part_count() + self.sections.chapters.len();
        match key {
            "UP" => self.sections.selection = self.sections.selection.saturating_sub(1),
            "DOWN" => {
                self.sections.selection = (self.sections.selection + 1).min(count.saturating_sub(1))
            }
            "LEFT" => self.sections.selection = self.sections.selection.saturating_sub(5),
            "RIGHT" => {
                self.sections.selection = (self.sections.selection + 5).min(count.saturating_sub(1))
            }
            "ESC" | "BACKSPACE" | "p" => self.sections.open = false,
            "r" => self.load_chapters(),
            "ENTER" | "SPACE" => {
                let selected = self.sections.selection;
                let part_count = self.sections.part_count();
                let (cid, start) = if selected < part_count {
                    (self.sections.parts[selected].cid, 0.0)
                } else if let Some(chapter) = self.sections.chapters.get(selected - part_count) {
                    (self.sections.cid, chapter.start)
                } else {
                    return;
                };
                self.sections.open = false;
                if self.page == Page::Player
                    && self
                        .current_play
                        .as_ref()
                        .is_some_and(|info| info.cid == cid)
                {
                    self.seek_pending = Some(start);
                    self.seek_deadline = Some(Instant::now());
                } else {
                    self.requested_play = Some(self.sections.bvid.clone());
                    self.requested_cid = Some(cid);
                    self.play_start = start;
                }
            }
            _ => return,
        }
        self.sections.dirty = true;
    }

    pub(super) fn sync_sections(&mut self) {
        let g_sections = self.ui.global::<crate::Sections>();
        g_sections.set_open(self.sections.open);
        if !self.sections.open {
            return;
        }
        if self.sections.dirty {
            self.sections.first = self
                .sections
                .first
                .min(self.sections.selection)
                .max(self.sections.selection.saturating_sub(4));
            let parts = self
                .sections
                .parts
                .iter()
                .take(self.sections.part_count())
                .enumerate()
                .map(|(index, part)| Cmt {
                    user: format!("P{} {}", index + 1, part.title).into(),
                    text: model::mmss(part.duration).into(),
                });
            let chapters = self.sections.chapters.iter().map(|chapter| Cmt {
                user: chapter.title.as_str().into(),
                text: model::mmss(chapter.start).into(),
            });
            g_sections.set_rows(ModelRc::new(VecModel::from(
                parts
                    .chain(chapters)
                    .skip(self.sections.first)
                    .take(5)
                    .collect::<Vec<_>>(),
            )));
            g_sections.set_selected((self.sections.selection - self.sections.first) as i32);
            self.sections.dirty = false;
        }
        g_sections.set_loading(self.sections.loading);
        g_sections.set_error(self.sections.error.as_str().into());
    }
}
