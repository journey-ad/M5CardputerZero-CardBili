use super::*;
use slint::ComponentHandle as _;

#[derive(Default)]
pub(super) struct Favorites {
    pub folders: Vec<model::FavoriteFolder>,
    pub covers: HashMap<i64, slint::Image>,
    pub selection: usize,
    pub first: usize,
    pub loaded: bool,
    pub loading: bool,
    pub error: String,
    pub request: u64,
    pub dirty: bool,
}

impl App {
    pub(super) fn favorites_loaded(
        &mut self,
        request: u64,
        result: Result<Vec<model::FavoriteFolder>, String>,
    ) {
        if request != self.favorites.request {
            return;
        }
        self.favorites.loading = false;
        match result {
            Ok(folders) => {
                let selected = self
                    .favorites
                    .folders
                    .get(self.favorites.selection)
                    .map(|folder| folder.id);
                if let Some(index) = folders
                    .iter()
                    .position(|folder| Some(folder.id) == selected)
                {
                    self.favorites.selection = index;
                }
                let missing = folders
                    .iter()
                    .filter(|folder| !self.favorites.covers.contains_key(&folder.id))
                    .cloned()
                    .collect();
                tasks::load_favorite_covers(missing, request, self.tx.clone());
                self.favorites.folders = folders;
                self.favorites.loaded = true;
                self.favorites.dirty = true;
            }
            Err(error) => self.favorites.error = error,
        }
    }

    pub(super) fn load_favorites(&mut self, force: bool) {
        if self.favorites.loading {
            return;
        }
        if let Some(cookies) = &self.cookies {
            self.favorites.loading = true;
            self.favorites.error.clear();
            self.favorites.request += 1;
            tasks::load_favorites(
                self.mid.clone(),
                cookies.clone(),
                self.favorites.request,
                force,
                self.tx.clone(),
            );
        }
    }

    pub(super) fn favorites_key(&mut self, key: &str) {
        let previous = self.favorites.selection;
        match key {
            "UP" => self.favorites.selection = previous.saturating_sub(1),
            "DOWN" => {
                self.favorites.selection =
                    (previous + 1).min(self.favorites.folders.len().saturating_sub(1))
            }
            "LEFT" => self.favorites.selection = previous.saturating_sub(4),
            "RIGHT" => {
                self.favorites.selection =
                    (previous + 4).min(self.favorites.folders.len().saturating_sub(1))
            }
            "ENTER" | "SPACE" => {
                self.requested_folder = self.favorites.folders.get(previous).cloned();
            }
            "r" => self.load_favorites(true),
            "ESC" | "BACKSPACE" => self.go_back = true,
            _ => {}
        }
        self.favorites.dirty |= previous != self.favorites.selection;
    }

    pub(super) fn sync_favorites(&mut self) {
        if self.page != Page::Favorites && !(self.page == Page::Home && self.tab == HomeTab::Mine) {
            return;
        }
        if !self.favorites.loaded && self.favorites.error.is_empty() {
            self.load_favorites(false);
        }
        let favorite = self.ui.global::<crate::Favorite>();
        if self.favorites.dirty {
            let total: u64 = self
                .favorites
                .folders
                .iter()
                .map(|folder| folder.count)
                .sum();
            self.ui
                .global::<crate::ListModel>()
                .set_favorite_count(format!("{total} 条").into());
            self.favorites.selection = self
                .favorites
                .selection
                .min(self.favorites.folders.len().saturating_sub(1));
            let selected = self.favorites.selection;
            self.favorites.first = self
                .favorites
                .first
                .min(selected)
                .max(selected.saturating_sub(3));
            favorite.set_folders(ModelRc::new(VecModel::from(
                self.favorites
                    .folders
                    .iter()
                    .skip(self.favorites.first)
                    .take(4)
                    .map(|folder| crate::FavoriteRow {
                        title: folder.title.as_str().into(),
                        count: format!("{} 条", folder.count).into(),
                        visibility: if folder.private { "私密" } else { "公开" }.into(),
                        cover: self
                            .favorites
                            .covers
                            .get(&folder.id)
                            .cloned()
                            .unwrap_or_default(),
                        has_cover: self.favorites.covers.contains_key(&folder.id),
                    })
                    .collect::<Vec<_>>(),
            )));
            favorite.set_selected((selected - self.favorites.first) as i32);
            self.favorites.dirty = false;
        }
        favorite.set_loading(self.favorites.loading);
        favorite.set_error(self.favorites.error.as_str().into());
    }
}
