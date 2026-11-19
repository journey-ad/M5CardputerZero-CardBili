use super::*;
use slint::ComponentHandle as _;
use std::sync::atomic::Ordering;

impl App {
    pub(super) fn open_cdn(&mut self) {
        let g_cdn = self.ui.global::<crate::Cdn>();
        g_cdn.set_open(true);
        let selected = crate::cdn::index(&self.settings.cdn) as i32;
        g_cdn.set_selected(selected);
        g_cdn.set_first(ensure_visible(0, selected as usize, self.cdn_visible()) as i32);
        self.test_cdns();
    }

    fn cdn_visible(&self) -> usize {
        let height = self.ui.global::<crate::Layout>().get_window_height();
        visible_rows(height, 48.0, 40.0)
    }

    pub(super) fn close_cdn(&self) {
        let cdn = self.ui.global::<crate::Cdn>();
        self.cdn_request.fetch_add(1, Ordering::Relaxed);
        cdn.set_open(false);
        cdn.set_testing(false);
    }

    fn test_cdns(&self) {
        let g_cdn = self.ui.global::<crate::Cdn>();
        if g_cdn.get_testing() {
            return;
        }
        let request = self.cdn_request.fetch_add(1, Ordering::Relaxed) + 1;
        let active = self.cdn_request.clone();
        g_cdn.set_testing(true);
        g_cdn.set_rows(ModelRc::new(VecModel::from(
            crate::cdn::OPTIONS
                .iter()
                .map(|cdn| Cmt {
                    user: crate::i18n::tr(cdn.name).into(),
                    text: crate::i18n::tr("等待测速…").into(),
                })
                .collect::<Vec<_>>(),
        )));
        let cookies = self.cookies.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            // 与 PiliPlus 使用同一测速样本
            let sample = crate::api::play_urls("BV1fK4y1t7hj", 196018899, cookies.as_deref())
                .and_then(|value| {
                    value["data"]["dash"]["video"]
                        .as_array()
                        .and_then(|streams| streams.first())
                        .cloned()
                        .ok_or_else(|| crate::i18n::tr("未获取到测速视频").into())
                });
            if let Err(error) = &sample {
                crate::diagnostics::write(format_args!("[CDN] 测速样本获取失败: {error}"));
            }
            for (index, cdn) in crate::cdn::OPTIONS.iter().enumerate() {
                if active.load(Ordering::Relaxed) != request {
                    return;
                }
                let _ = tx.send(Msg::CdnSpeed(request, index, crate::i18n::tr("测速中…").into()));
                let text = match &sample {
                    Ok(stream) => cdn.measure(stream),
                    Err(_) => crate::i18n::tr("样本加载失败，按 R 重试").into(),
                };
                if tx.send(Msg::CdnSpeed(request, index, text)).is_err() {
                    return;
                }
            }
            let _ = tx.send(Msg::CdnSpeed(
                request,
                crate::cdn::OPTIONS.len(),
                String::new(),
            ));
        });
    }

    pub(super) fn cdn_result(&self, request: u64, index: usize, result: String) {
        let g_cdn = self.ui.global::<crate::Cdn>();
        use slint::Model;
        if self.cdn_request.load(Ordering::Relaxed) != request {
            return;
        }
        if index == crate::cdn::OPTIONS.len() {
            g_cdn.set_testing(false);
        } else {
            g_cdn.get_rows().set_row_data(
                index,
                Cmt {
                    user: crate::i18n::tr(crate::cdn::OPTIONS[index].name).into(),
                    text: result.into(),
                },
            );
        }
    }

    pub(super) fn cdn_key(&mut self, key: &str) {
        let g_cdn = self.ui.global::<crate::Cdn>();
        let selected = g_cdn.get_selected();
        let last = crate::cdn::OPTIONS.len() as i32 - 1;
        match key {
            "UP" => g_cdn.set_selected((selected - 1).max(0)),
            "DOWN" => g_cdn.set_selected((selected + 1).min(last)),
            "LEFT" => g_cdn.set_selected((selected - 3).max(0)),
            "RIGHT" => g_cdn.set_selected((selected + 3).min(last)),
            "ENTER" | "SPACE" => {
                self.settings.cdn = crate::cdn::OPTIONS[selected as usize].id.into();
                self.settings_changed = true;
                self.close_cdn();
            }
            "r" => self.test_cdns(),
            "ESC" | "BACKSPACE" => self.close_cdn(),
            _ => {}
        }
        if matches!(key, "UP" | "DOWN" | "LEFT" | "RIGHT") {
            g_cdn.set_first(ensure_visible(
                g_cdn.get_first() as usize,
                g_cdn.get_selected() as usize,
                self.cdn_visible(),
            ) as i32);
        }
    }
}
