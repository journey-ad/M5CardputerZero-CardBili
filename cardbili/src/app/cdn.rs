use super::*;
use std::sync::atomic::Ordering;

impl App {
    pub(super) fn open_cdn(&mut self) {
        self.ui.set_cdn_open(true);
        self.ui
            .set_cdn_selected(crate::cdn::index(&self.settings.cdn) as i32);
        self.test_cdns();
    }

    pub(super) fn close_cdn(&self) {
        self.cdn_request.fetch_add(1, Ordering::Relaxed);
        self.ui.set_cdn_open(false);
        self.ui.set_cdn_testing(false);
    }

    fn test_cdns(&self) {
        if self.ui.get_cdn_testing() {
            return;
        }
        let request = self.cdn_request.fetch_add(1, Ordering::Relaxed) + 1;
        let active = self.cdn_request.clone();
        self.ui.set_cdn_testing(true);
        self.ui.set_cdn_rows(ModelRc::new(VecModel::from(
            crate::cdn::OPTIONS
                .iter()
                .map(|cdn| Cmt {
                    user: cdn.name.into(),
                    text: "等待测速…".into(),
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
                        .ok_or_else(|| "未获取到测速视频".into())
                });
            if let Err(error) = &sample {
                crate::diagnostics::write(format_args!("[CDN] 测速样本获取失败: {error}"));
            }
            for (index, cdn) in crate::cdn::OPTIONS.iter().enumerate() {
                if active.load(Ordering::Relaxed) != request {
                    return;
                }
                let _ = tx.send(Msg::CdnSpeed(request, index, "测速中…".into()));
                let text = match &sample {
                    Ok(stream) => cdn.measure(stream),
                    Err(_) => "样本加载失败，按 R 重试".into(),
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
        use slint::Model;
        if self.cdn_request.load(Ordering::Relaxed) != request {
            return;
        }
        if index == crate::cdn::OPTIONS.len() {
            self.ui.set_cdn_testing(false);
        } else {
            self.ui.get_cdn_rows().set_row_data(
                index,
                Cmt {
                    user: crate::cdn::OPTIONS[index].name.into(),
                    text: result.into(),
                },
            );
        }
    }

    pub(super) fn cdn_key(&mut self, key: &str) {
        let selected = self.ui.get_cdn_selected();
        let last = crate::cdn::OPTIONS.len() as i32 - 1;
        match key {
            "UP" => self.ui.set_cdn_selected((selected - 1).max(0)),
            "DOWN" => self.ui.set_cdn_selected((selected + 1).min(last)),
            "LEFT" => self.ui.set_cdn_selected((selected - 3).max(0)),
            "RIGHT" => self.ui.set_cdn_selected((selected + 3).min(last)),
            "ENTER" | "SPACE" => {
                self.settings.cdn = crate::cdn::OPTIONS[selected as usize].id.into();
                self.settings_changed = true;
                self.close_cdn();
            }
            "r" => self.test_cdns(),
            "ESC" | "BACKSPACE" => self.close_cdn(),
            _ => {}
        }
    }
}
