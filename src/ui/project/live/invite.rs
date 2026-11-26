//! Presenta la invitación y pide copiarla desde un gesto explícito del usuario.
use super::DawApp;
use crate::ui::components::modal::{ModalClose, modal_backdrop};
use gpui::{
    AppContext, ClipboardItem, Context, Div, Entity, Window,
    component::{
        Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputState},
    },
    div,
    prelude::*,
    px, rgb,
};
use std::rc::Rc;

#[derive(Default)]
pub(in crate::ui::project) struct ShareInvite {
    field: Option<Entity<InputState>>,
    copied: bool,
    error: Option<String>,
}

impl DawApp {
    pub(in crate::ui) fn shared_invite_open(&self) -> bool {
        self.project.share_invite.is_some() || self.project.identity_dialog.is_some()
    }

    pub(in crate::ui) fn close_shared_invite(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.project.share_invite = None;
        self.project.identity_dialog = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    pub(super) fn show_share_error(&mut self, message: String) {
        if let Some(dialog) = &mut self.project.share_invite {
            dialog.error = Some(message.clone());
        }
        self.status_message = message;
    }

    pub(in crate::ui) fn render_shared_invite(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        if self.project.identity_dialog.is_some() {
            return self.render_identity_dialog(cx);
        }
        let dialog = self.project.share_invite.as_mut()?;
        let invite = self
            .project
            .shared
            .as_ref()
            .and_then(|shared| shared.invite.as_ref());
        let field = invite.map(|invite| {
            dialog
                .field
                .get_or_insert_with(|| {
                    cx.new(|cx| {
                        InputState::new(window, cx).default_value(invite.as_str().to_owned())
                    })
                })
                .clone()
        });
        let message = if let Some(error) = &dialog.error {
            error.as_str()
        } else if dialog.copied {
            "Invitación copiada"
        } else if field.is_some() {
            "Envía este enlace a tu colaborador. Puede unirse directo desde una sesión en blanco; los audios viajan solos."
        } else {
            "Preparando invitación…"
        };
        let ds = &self.ds;
        let on_close: ModalClose = Rc::new(cx.listener(|this, _, window, cx| {
            this.close_shared_invite(window, cx);
            cx.stop_propagation();
        }));
        let card = div()
            .w((window.viewport_size().width - px(32.0))
                .max(px(1.0))
                .min(px(550.0)))
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(rgb(ds.panel_dark))
            .border_1()
            .border_color(rgb(ds.border))
            .rounded(px(ds.radius_lg))
            .child(div().text_lg().child("Compartir sesión"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(if dialog.error.is_some() {
                        ds.pink
                    } else {
                        ds.muted
                    }))
                    .child(message.to_owned()),
            )
            .when_some(field, |card, input| {
                card.child(Input::new(&input).readonly(true).w_full())
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("close-share-invite")
                            .small()
                            .label("Cerrar")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_shared_invite(window, cx)
                            })),
                    )
                    .when(invite.is_some() && dialog.error.is_none(), |row| {
                        row.child(
                            Button::new("copy-share-invite")
                                .primary()
                                .small()
                                .label("Copiar invitación")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(invite) = this
                                        .project
                                        .shared
                                        .as_ref()
                                        .and_then(|shared| shared.invite.as_ref())
                                    {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            invite.clone(),
                                        ));
                                        if let Some(dialog) = &mut this.project.share_invite {
                                            dialog.copied = true;
                                        }
                                        this.status_message = "Invitación copiada".into();
                                        cx.notify();
                                    }
                                })),
                        )
                    }),
            );
        Some(modal_backdrop(0x05070bd9, on_close, card))
    }
}
