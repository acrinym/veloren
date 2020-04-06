//! A widget for selecting a single value along some linear range.
use conrod_core::{
    builder_methods, image,
    text::font,
    widget::{self, Image, Text},
    widget_ids, Color, Colorable, Positionable, Sizeable, Widget, WidgetCommon,
};
use vek::*;

const AMOUNT_SHADOW_OFFSET: [f64; 2] = [1.0, 1.0];

pub trait ContentKey: Copy {
    type ContentSource;
    type ImageSource;
    type ImageKey: PartialEq + Send + 'static;
    /// Returns an Option since the slot could be empty
    fn image_key(&self, source: &Self::ContentSource) -> Option<Self::ImageKey>;
    // TODO: is this the right integer type?
    fn amount(&self, source: &Self::ContentSource) -> Option<u32>;
    fn image_id(key: &Self::ImageKey, source: &Self::ImageSource) -> image::Id;
}

pub trait SlotKinds: Sized + PartialEq + Copy {}

pub struct ContentSize {
    // Width divided by height
    pub width_height_ratio: f32,
    // Max fraction of slot widget size that each side can be
    pub max_fraction: f32,
}

pub struct SlotMaker<'a, C: ContentKey + Into<K>, K: SlotKinds> {
    pub empty_slot: image::Id,
    pub filled_slot: image::Id,
    pub selected_slot: image::Id,
    // Is this useful?
    pub background_color: Option<Color>,
    pub content_size: ContentSize,
    // How to scale content size relative to base content size when selected
    pub selected_content_scale: f32,
    pub amount_font: font::Id,
    pub amount_font_size: u32,
    pub amount_margins: Vec2<f32>,
    pub amount_text_color: Color,
    pub content_source: &'a C::ContentSource,
    pub image_source: &'a C::ImageSource,
    pub slot_manager: Option<&'a mut SlotManager<K>>,
}

impl<'a, C, K> SlotMaker<'a, C, K>
where
    C: ContentKey + Into<K>,
    K: SlotKinds,
{
    pub fn fabricate(&mut self, contents: C, wh: [f32; 2]) -> Slot<C, K> {
        let content_size = {
            let ContentSize {
                max_fraction,
                width_height_ratio,
            } = self.content_size;
            let w_max = max_fraction * wh[0];
            let h_max = max_fraction * wh[1];
            let max_ratio = w_max / h_max;
            let (w, h) = if max_ratio > width_height_ratio {
                (width_height_ratio * h_max, w_max)
            } else {
                (w_max, w_max / width_height_ratio)
            };
            Vec2::new(w, h)
        };
        Slot::new(
            contents,
            self.empty_slot,
            self.filled_slot,
            self.selected_slot,
            content_size,
            self.selected_content_scale,
            self.amount_font,
            self.amount_font_size,
            self.amount_margins,
            self.amount_text_color,
            self.content_source,
            self.image_source,
        )
        .wh([wh[0] as f64, wh[1] as f64])
        .and_then(self.background_color, |s, c| s.with_background_color(c))
        .and_then(self.slot_manager.as_mut(), |s, m| s.with_manager(m))
    }
}

/*
// Note: will probably delete this
// Integrate tooltip adding??
use super::tooltip::{Tooltip, TooltipManager, Tooltipable};
pub fn with_tooltips(
    self,
    tooltip_manager: &'a mut TooltipManager,
    tooltip: &'a Tooltip<'a>,
) -> TooltippedSlotMaker<'a, C, K> {
    TooltippedSlotMaker {
        slot_maker: self,
        tooltip_manager,
        tooltip,
    }
}


tooltip: &'a Tooltip<'a>,
slot_widget
    .with_tooltip(
        self.tooltip_manager,
        &item.name(),
        &format!(
            "{}",
            /* item.kind, item.effect(), */ item.description()
        ),
        &item_tooltip,
    )

pub struct TooltippedSlotMaker<'a, C: ContentKey + Into<K>, K: SlotKinds> {
    pub slot_maker: SlotMaker<'a, C, K>,
    pub tooltip_manager: &'a mut TooltipManager,
    pub tooltip: &'a Tooltip,
}*/

#[derive(Clone, Copy)]
enum ManagerState<K> {
    Dragging(widget::Id, K),
    Selected(widget::Id, K),
    Idle,
}

enum Interaction {
    Selected,
    Dragging,
    None,
}

pub enum Event<K> {
    // Dragged to another slot
    Dragged(K, K),
    // Dragged to open space
    Dropped(K),
    // Clicked while selected
    Used(K),
}
// Handles interactions with slots
pub struct SlotManager<K: SlotKinds> {
    state: ManagerState<K>,
    events: Vec<Event<K>>,
    // widget id for dragging image
}

impl<K> SlotManager<K>
where
    K: SlotKinds,
{
    pub fn new() -> Self {
        Self {
            state: ManagerState::Idle,
            events: Vec::new(),
        }
    }

    pub fn maintain(&mut self, ui: &conrod_core::Ui) -> Vec<Event<K>> {
        // Detect drops by of selected item by clicking in empty space
        if let ManagerState::Selected(_, slot) = self.state {
            if ui.widget_input(ui.window).clicks().left().next().is_some() {
                self.state = ManagerState::Idle;
                self.events.push(Event::Dropped(slot))
            }
        }

        std::mem::replace(&mut self.events, Vec::new())
    }

    fn update(
        &mut self,
        widget: widget::Id,
        slot: K,
        ui: &conrod_core::Ui,
        filled: bool,
    ) -> Interaction {
        // If the slot is no longer filled deselect it or cancel dragging
        match &self.state {
            ManagerState::Selected(id, _) | ManagerState::Dragging(id, _)
                if *id == widget && !filled =>
            {
                self.state = ManagerState::Idle;
            }
            _ => (),
        }

        // If this is the selected/dragged widget make sure the slot value is up to date
        match &mut self.state {
            ManagerState::Selected(id, stored_slot) | ManagerState::Dragging(id, stored_slot)
                if *id == widget =>
            {
                *stored_slot = slot
            },
            _ => (),
        }

        // TODO: make more robust wrt multiple events in the same frame (eg event order
        // may matter) TODO: handle taps as well
        // TODO: handle drags
        let click_count = ui.widget_input(widget).clicks().left().count();
        if click_count > 0 {
            let odd_num_clicks = click_count % 2 == 1;
            self.state = if let ManagerState::Selected(id, other_slot) = self.state {
                if id != widget {
                    // Swap
                    if slot != other_slot {
                        self.events.push(Event::Dragged(other_slot, slot));
                    }
                    if click_count == 1 {
                        ManagerState::Idle
                    } else if click_count == 2 {
                        // Was clicked again
                        ManagerState::Selected(widget, slot)
                    } else {
                        // Clicked more than once after swap, use and deselect
                        self.events.push(Event::Used(slot));
                        ManagerState::Idle
                    }
                } else {
                    // Clicked widget was already selected
                    // Deselect and emit use if clicked while selected
                    self.events.push(Event::Used(slot));
                    ManagerState::Idle
                }
            } else {
                // No widgets were selected
                if odd_num_clicks && filled {
                    ManagerState::Selected(widget, slot)
                } else {
                    // Selected and then deselected with one or more clicks
                    ManagerState::Idle
                }
            };
        }

        // Determine whether this slot is being interacted with
        match self.state {
            ManagerState::Selected(id, _) if id == widget => Interaction::Selected,
            ManagerState::Dragging(id, _) if id == widget => Interaction::Dragging,
            _ => Interaction::None,
        }
    }
}

#[derive(WidgetCommon)]
pub struct Slot<'a, C: ContentKey + Into<K>, K: SlotKinds> {
    content: C,

    // Images for slot background and frame
    empty_slot: image::Id,
    filled_slot: image::Id,
    selected_slot: image::Id,
    background_color: Option<Color>,

    // Size of content image
    content_size: Vec2<f32>,
    selected_content_scale: f32,

    icon: Option<(image::Id, Vec2<f32>, Option<Color>)>,

    // Amount styling
    amount_font: font::Id,
    amount_font_size: u32,
    amount_margins: Vec2<f32>,
    amount_text_color: Color,

    slot_manager: Option<&'a mut SlotManager<K>>,
    // Should we just pass in the ImageKey?
    content_source: &'a C::ContentSource,
    image_source: &'a C::ImageSource,

    #[conrod(common_builder)]
    common: widget::CommonBuilder,
}

widget_ids! {
    // Note: icon, amount, and amount_bg are not always used. Is there any cost to having them?
    struct Ids {
        background,
        icon,
        amount,
        amount_bg,
        content,
    }
}

/// Represents the state of the Slot widget.
pub struct State<K> {
    ids: Ids,
    cached_image: Option<(K, image::Id)>,
}

impl<'a, C, K> Slot<'a, C, K>
where
    C: ContentKey + Into<K>,
    K: SlotKinds,
{
    builder_methods! {
        pub with_manager { slot_manager = Some(&'a mut SlotManager<K>) }
        pub with_background_color { background_color = Some(Color) }
    }

    pub fn with_icon(mut self, img: image::Id, size: Vec2<f32>, color: Option<Color>) -> Self {
        self.icon = Some((img, size, color));
        self
    }

    fn new(
        content: C,
        empty_slot: image::Id,
        filled_slot: image::Id,
        selected_slot: image::Id,
        content_size: Vec2<f32>,
        selected_content_scale: f32,
        amount_font: font::Id,
        amount_font_size: u32,
        amount_margins: Vec2<f32>,
        amount_text_color: Color,
        content_source: &'a C::ContentSource,
        image_source: &'a C::ImageSource,
    ) -> Self {
        Self {
            content,
            empty_slot,
            filled_slot,
            selected_slot,
            background_color: None,
            content_size,
            selected_content_scale,
            icon: None,
            amount_font,
            amount_font_size,
            amount_margins,
            amount_text_color,
            slot_manager: None,
            content_source,
            image_source,
            common: widget::CommonBuilder::default(),
        }
    }
}

impl<'a, C, K> Widget for Slot<'a, C, K>
where
    C: ContentKey + Into<K>,
    K: SlotKinds,
{
    type Event = ();
    type State = State<C::ImageKey>;
    type Style = ();

    fn init_state(&self, id_gen: widget::id::Generator) -> Self::State {
        State {
            ids: Ids::new(id_gen),
            cached_image: None,
        }
    }

    fn style(&self) -> Self::Style { () }

    /// Update the state of the Slider.
    fn update(self, args: widget::UpdateArgs<Self>) -> Self::Event {
        let widget::UpdateArgs {
            id,
            state,
            rect,
            ui,
            ..
        } = args;
        let Slot {
            content,
            empty_slot,
            filled_slot,
            selected_slot,
            background_color,
            content_size,
            selected_content_scale,
            icon,
            amount_font,
            amount_font_size,
            amount_margins,
            amount_text_color,
            content_source,
            image_source,
            ..
        } = self;

        // If the key changed update the cached image id
        let image_key = content.image_key(content_source);
        if state.cached_image.as_ref().map(|c| &c.0) != image_key.as_ref() {
            state.update(|state| {
                state.cached_image = image_key.map(|key| {
                    let image_id = C::image_id(&key, &image_source);
                    (key, image_id)
                });
            });
        }

        // Get whether this slot is selected
        let interaction = self.slot_manager.map_or(Interaction::None, |m| {
            m.update(id, content.into(), ui, state.cached_image.is_some())
        });

        // Get image ids
        let content_image = state.cached_image.as_ref().map(|c| c.1);
        let slot_image = if let Interaction::Selected = interaction {
            selected_slot
        } else if content_image.is_some() {
            filled_slot
        } else {
            empty_slot
        };

        // Get amount (None => no amount text)
        let amount = content.amount(content_source);

        // Get slot widget dimensions and position
        let (x, y, w, h) = rect.x_y_w_h();

        // Draw slot frame/background
        Image::new(slot_image)
            .x_y(x, y)
            .w_h(w, h)
            .parent(id)
            .graphics_for(id)
            .color(background_color)
            .set(state.ids.background, ui);

        // Draw icon (only when there is not content)
        // Note: this could potentially be done by the user instead
        if let (Some((icon_image, size, color)), true) = (icon, content_image.is_none()) {
            let wh = size.map(|e| e as f64).into_array();
            Image::new(icon_image)
                .x_y(x, y)
                .wh(wh)
                .parent(id)
                .graphics_for(id)
                .color(color)
                .set(state.ids.icon, ui);
        }

        // Draw contents
        if let Some(content_image) = content_image {
            Image::new(content_image)
                .x_y(x, y)
                .wh((content_size
                    * if let Interaction::Selected = interaction {
                        selected_content_scale
                    } else {
                        1.0
                    })
                .map(|e| e as f64)
                .into_array())
                .parent(id)
                .graphics_for(id)
                .set(state.ids.content, ui);
        }

        // Draw amount
        if let Some(amount) = amount {
            let amount = format!("{}", &amount);
            // Text shadow
            Text::new(&amount)
                .font_id(amount_font)
                .font_size(amount_font_size)
                .top_right_with_margins_on(
                    state.ids.content,
                    amount_margins.x as f64,
                    amount_margins.y as f64,
                )
                .parent(id)
                .graphics_for(id)
                .color(Color::Rgba(0.0, 0.0, 0.0, 1.0))
                .set(state.ids.amount_bg, ui);
            Text::new(&amount)
                .parent(id)
                .graphics_for(id)
                .bottom_left_with_margins_on(
                    state.ids.amount_bg,
                    AMOUNT_SHADOW_OFFSET[0],
                    AMOUNT_SHADOW_OFFSET[1],
                )
                .font_id(amount_font)
                .font_size(amount_font_size)
                .color(amount_text_color)
                .set(state.ids.amount, ui);
        }
    }
}
