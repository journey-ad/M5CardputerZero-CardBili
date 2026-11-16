use super::*;
use slint::ComponentHandle as _;

impl App {
    pub(super) fn handle_message(&mut self, m: Msg) {
        let detail = self.ui.global::<crate::Detail>();
        match m {
            Msg::Confirmed(accepted) => self.finish_confirmation(accepted),
            Msg::CacheUsage(result) => self.cache_usage(result),
            Msg::CacheCleared(result) => self.cache_cleared(result),
            Msg::CdnSpeed(request, index, result) => self.cdn_result(request, index, result),
            Msg::Chapters(cid, result) => self.chapters_loaded(cid, result),
            Msg::Watching(request, Ok(text)) if request == self.play_request => {
                self.ui.global::<crate::Player>().set_watching(text.into())
            }
            Msg::Watching(_, _) => {}
            Msg::Liked(request, result) => self.liked_loaded(request, result),
            Msg::Reaction(request, result) => self.reaction_finished(request, result),
            Msg::Favorites(request, result) => self.favorites_loaded(request, result),
            Msg::FavoriteCover(request, id, (w, h, pixels)) => {
                if request == self.favorites.request {
                    self.favorites
                        .covers
                        .insert(id, util::raw_to_image(w, h, &pixels));
                    self.favorites.dirty = true;
                }
            }
            Msg::FollowingTags(request, result) => self.following_tags_loaded(request, result),
            Msg::FollowingGroup(request, tagid, page, result) => {
                self.following_group_loaded(request, tagid, page, result)
            }
            Msg::Dynamic(request, offset, result) => self.dynamic_loaded(request, offset, result),
            Msg::History(request, first, result) => self.history_loaded(request, first, result),
            Msg::Comments(request, page, result) => self.comments_loaded(request, page, result),
            Msg::CommentEmotes(images) => crate::emoji::register_emotes(&self.ui, images),
            Msg::Volume(result) => {
                if self.page == Page::Player {
                    match result {
                        Ok(volume) => self.toast(&format!("系统音量 {volume}%")),
                        Err(_) => {
                            self.toast("系统音量调整失败");
                        }
                    }
                }
            }
            Msg::Frame(g) => self.frame_ready(g),
            Msg::PlaybackEnded(g) => self.playback_ended(g),
            Msg::PlaybackFailed(g, error) => self.playback_failed(g, error),
            Msg::ListLoaded(t, items) => self.list_loaded(t, items),
            Msg::ListMore(t, items) => self.list_more(t, items),
            Msg::Err(t, msg) => self.list_error(t, msg),
            Msg::PlayReady(request, result) => self.play_ready(request, result),
            Msg::LiveReady(request, result) => self.live_ready(request, result),
            Msg::LiveAreas(result) => self.live_areas_loaded(result),
            Msg::LiveList(request, page, result) => self.live_list_loaded(request, page, result),
            Msg::Danmaku(g, eng) => self.danmaku_loaded(g, eng),
            Msg::QrReady(request, url) => self.qr_ready(request, url),
            Msg::QrStatus(request, s) => self.qr_status(request, s),
            Msg::QrDone(request, session) => self.login_finished(request, session),
            Msg::Acct(account, avatar) => {
                self.account_dirty = true;
                self.avatar_img = avatar.map(|(w, h, raw)| util::raw_to_image(w, h, &raw));
                self.acct = Some(account);
            }
            Msg::FollowingAvatar(request, mid, (w, h, raw)) => {
                if request == self.following_request {
                    self.following
                        .avatars
                        .insert(mid, util::raw_to_image(w, h, &raw));
                    self.following.dirty = true;
                }
            }
            Msg::Cover(t, bvid, w, h, raw) => self.cover_loaded(t, bvid, (w, h, raw)),
            Msg::Detail(request, data, w, h, raw) => self.detail_loaded(request, data, (w, h, raw)),
            Msg::DetailError(request, error) => {
                if self.page == Page::Detail && request == self.detail_request {
                    detail.set_loading(false);
                    detail.set_error(error.into());
                }
            }
            Msg::AuthorProfile(request, result) => self.author_profile_loaded(request, result),
            Msg::Collection(request, loaded_page, result) => {
                self.collection_loaded(request, loaded_page, result)
            }
        }
    }
}
