use super::*;

impl App {
    pub(super) fn sync_cache(&mut self) {
        if self.page != Page::Home
            || self.tab != HomeTab::Settings
            || self.ui.get_cache_busy()
            || self.confirmation.is_some()
        {
            return;
        }
        if self
            .cache_checked
            .is_some_and(|time| time.elapsed() < Duration::from_secs(2))
        {
            return;
        }
        self.cache_checked = Some(Instant::now());
        self.ui.set_cache_busy(true);
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = crate::cache::usage().map_err(|error| error.to_string());
            let _ = tx.send(Msg::CacheUsage(result));
        });
    }

    pub(super) fn cache_usage(&self, result: Result<u64, String>) {
        self.ui.set_cache_busy(false);
        let text = match result {
            Ok(bytes) => format!("{:.1} MB / 1 GB", bytes as f64 / 1024.0 / 1024.0),
            Err(error) => {
                crate::diagnostics::write(format_args!("[缓存] 统计失败: {error}"));
                "统计失败".into()
            }
        };
        self.ui.set_cache_size(text.into());
    }

    pub(super) fn clear_cache(&self) {
        if self.ui.get_cache_busy() {
            return;
        }
        self.ui.set_cache_busy(true);
        self.ui.set_cache_size("清理中…".into());
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = crate::cache::clear().map_err(|error| error.to_string());
            let _ = tx.send(Msg::CacheCleared(result));
        });
    }

    pub(super) fn cache_cleared(&mut self, result: Result<(), String>) {
        self.ui.set_cache_busy(false);
        self.cache_checked = None;
        match result {
            Ok(()) => {
                self.ui.set_cache_size("0.0 MB / 1 GB".into());
                self.toast("缓存已清理");
            }
            Err(error) => {
                crate::diagnostics::write(format_args!("[缓存] 清理失败: {error}"));
                self.toast("缓存清理失败");
            }
        }
    }
}
