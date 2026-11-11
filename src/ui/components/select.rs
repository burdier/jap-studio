//! Select tipado compartido: Kit posee búsqueda, popup, teclado y foco.
//! Sin dependencias del DAW, configuración o motor de audio.
use gpui::{
    App, AppContext, Entity, FocusHandle, Focusable, IntoElement, SharedString, Window,
    component::{
        Sizable,
        select::{SearchableVec, Select, SelectItem, SelectState},
    },
    prelude::*,
    px,
};

#[derive(Clone, PartialEq)]
pub(in crate::ui) struct SelectOption<T: Clone + PartialEq> {
    pub label: String,
    pub value: T,
    pub display_label: Option<String>,
}
impl<T: Clone + PartialEq> SelectItem for SelectOption<T> {
    type Value = T;
    fn title(&self) -> SharedString {
        self.label.clone().into()
    }
    fn value(&self) -> &T {
        &self.value
    }
    fn display_title(&self) -> Option<gpui::AnyElement> {
        self.display_label.as_ref().map(|label| {
            gpui::div()
                .min_w_0()
                .truncate()
                .child(label.clone())
                .into_any_element()
        })
    }
}
pub(in crate::ui) type SelectItems<T> = SearchableVec<SelectOption<T>>;

pub(in crate::ui) struct SelectControl<T: Clone + PartialEq + 'static> {
    pub state: Entity<SelectState<SelectItems<T>>>,
    options: Vec<SelectOption<T>>,
    trigger: FocusHandle,
}
impl<T: Clone + PartialEq + 'static> SelectControl<T> {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        let state = cx.new(|cx| {
            SelectState::new(
                SearchableVec::from(Vec::<SelectOption<T>>::new()),
                None,
                window,
                cx,
            )
            .searchable(true)
        });
        let trigger = state.read(cx).focus_handle(cx);
        Self {
            state,
            options: Vec::new(),
            trigger,
        }
    }
    /// No reiniciar búsqueda ni cursor en cada frame del medidor.
    pub fn sync(
        &mut self,
        options: Vec<SelectOption<T>>,
        selected: Option<&T>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.options != options {
            self.options = options.clone();
            self.state.update(cx, |state, cx| {
                state.set_items(SearchableVec::from(options), window, cx)
            });
        }
        if self.state.read(cx).selected_value() != selected {
            self.state.update(cx, |state, cx| match selected {
                Some(value) => state.set_selected_value(value, window, cx),
                None => state.set_selected_index(None, window, cx),
            });
        }
    }
    pub fn render(
        &self,
        label: &'static str,
        placeholder: String,
        disabled: bool,
    ) -> impl IntoElement {
        Select::new(&self.state)
            .small()
            .w_full()
            .menu_max_h(px(260.))
            .accessibility_label(label)
            .placeholder(placeholder)
            .search_placeholder("Buscar…")
            .disabled(disabled)
    }
    pub fn focused(&self, window: &Window, cx: &App) -> bool {
        self.trigger.is_focused(window) || self.popup_focused(window, cx)
    }
    pub fn popup_focused(&self, window: &Window, cx: &App) -> bool {
        let focus = self.state.read(cx).focus_handle(cx);
        focus != self.trigger && focus.contains_focused(window, cx)
    }
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.trigger.focus(window, cx);
    }
}
