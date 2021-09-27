use anyhow::{bail, Result};
use derivative::Derivative;
use heck::TitleCase;
use iced::{
    button, scrollable, svg, text_input, tooltip, Align, Button, Color, Column, Command, Container,
    Length, Row, Scrollable, Svg, Text, Tooltip,
};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use strum::Display;
use tracing::error;

use bl3_save_edit_core::bl3_item::{
    BalancePart, Bl3Item, InvDataPart, ManufacturerPart, MAX_BL3_ITEM_ANOINTMENTS,
    MAX_BL3_ITEM_PARTS,
};
use bl3_save_edit_core::bl3_profile::Bl3Profile;
use bl3_save_edit_core::bl3_save::character_data::MAX_CHARACTER_LEVEL;
use bl3_save_edit_core::bl3_save::Bl3Save;
use bl3_save_edit_core::resources::{INVENTORY_SERIAL_DB, LOOTLEMON_ITEMS};

use crate::bl3_ui::{Bl3Message, InteractionMessage, MessageResult};
use crate::bl3_ui_style::{Bl3UiStyle, Bl3UiTooltipStyle};
use crate::commands::interaction;
use crate::resources::fonts::{JETBRAINS_MONO, JETBRAINS_MONO_BOLD};
use crate::resources::svgs::{ARROW_DOWN, ARROW_UP};
use crate::util::ErrorExt;
use crate::views::item_editor::available_parts::AvailablePartTypeIndex;
use crate::views::item_editor::current_parts::CurrentPartTypeIndex;
use crate::views::item_editor::item_editor_list_item::ItemEditorListItem;
use crate::views::item_editor::item_editor_lootlemon_item::ItemEditorLootlemonItem;
use crate::views::item_editor::parts_tab_bar::{AvailablePartType, CurrentPartType};
use crate::views::tab_bar_button::tab_bar_button;
use crate::views::{InteractionExt, NO_SEARCH_RESULTS_FOUND_MESSAGE};
use crate::widgets::labelled_element::LabelledElement;
use crate::widgets::notification::{Notification, NotificationSentiment};
use crate::widgets::number_input::NumberInput;
use crate::widgets::text_input_limited::TextInputLimited;

pub mod available_parts;
pub mod current_parts;
pub mod editor;
pub mod extra_part_info;
pub mod item_button_style;
pub mod item_editor_list_item;
pub mod item_editor_lootlemon_item;
pub mod list_item_contents;
pub mod parts_tab_bar;

#[derive(Derivative)]
#[derivative(Debug, Default)]
pub struct ItemEditorState {
    pub selected_item_index: usize,
    pub create_item_button_state: button::State,
    pub import_serial_input: String,
    pub import_serial_input_state: text_input::State,
    #[derivative(Default(value = "1"))]
    pub all_item_levels_input: i32,
    pub all_item_levels_input_state: text_input::State,
    pub all_item_levels_button_state: button::State,
    pub import_serial_button_state: button::State,
    items: Vec<ItemEditorListItem>,
    lootlemon_items: ItemEditorLootlemonItems,
    pub search_items_input_state: text_input::State,
    pub search_lootlemon_items_input_state: text_input::State,
    pub search_items_input: String,
    pub search_lootlemon_items_input: String,
    pub item_list_scrollable_state: scrollable::State,
    pub item_list_lootlemon_scrollable_state: scrollable::State,
    pub item_list_tab_type: ItemListTabType,
    pub item_list_items_tab_button_state: button::State,
    pub item_list_reverse_order_button_state: button::State,
    pub item_list_is_reverse_order: bool,
    pub item_list_lootlemon_tab_button_state: button::State,
}

#[derive(Debug)]
pub struct ItemEditorLootlemonItems {
    pub items: Vec<ItemEditorLootlemonItem>,
}

impl std::default::Default for ItemEditorLootlemonItems {
    fn default() -> Self {
        let items = LOOTLEMON_ITEMS
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, lootlemon_item)| {
                ItemEditorLootlemonItem::new(i, lootlemon_item.link, lootlemon_item.item)
            })
            .collect::<Vec<_>>();

        Self { items }
    }
}

impl ItemEditorState {
    pub fn items(&mut self) -> &Vec<ItemEditorListItem> {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut Vec<ItemEditorListItem> {
        &mut self.items
    }

    pub fn add_item(&mut self, item: Bl3Item) {
        self.items.push(ItemEditorListItem::new(item));
    }

    pub fn insert_item(&mut self, index: usize, item: Bl3Item) {
        if index < self.items.len() {
            self.items.insert(index, ItemEditorListItem::new(item));
        }
    }

    pub fn remove_item(&mut self, remove_id: usize) {
        if remove_id < self.items.len() {
            self.items.remove(remove_id);
        }
    }
}

pub trait ItemEditorStateExt {
    fn map_current_item_if_exists<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut ItemEditorListItem);

    fn map_current_item_if_exists_result<F>(&mut self, f: F) -> Result<&mut ItemEditorListItem>
    where
        F: FnOnce(&mut ItemEditorListItem) -> Result<()>;

    fn map_current_item_if_exists_to_editor_state(&mut self) -> Result<()>;
}

impl ItemEditorStateExt for ItemEditorState {
    fn map_current_item_if_exists<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut ItemEditorListItem),
    {
        if let Some(item) = self.items.get_mut(self.selected_item_index) {
            f(item);

            self.map_current_item_if_exists_to_editor_state()?;

            Ok(())
        } else {
            let msg = format!("couldn't get item for index: {}", self.selected_item_index);

            bail!("{}", msg);
        }
    }

    fn map_current_item_if_exists_result<F>(&mut self, f: F) -> Result<&mut ItemEditorListItem>
    where
        F: FnOnce(&mut ItemEditorListItem) -> Result<()>,
    {
        if let Some(item) = self.items.get_mut(self.selected_item_index) {
            f(item)?;

            item.map_item_to_editor()?;

            Ok(item)
        } else {
            let msg = format!("couldn't get item for index: {}", self.selected_item_index);

            bail!("{}", msg);
        }
    }

    fn map_current_item_if_exists_to_editor_state(&mut self) -> Result<()> {
        if let Some(curr_item) = self.items.get_mut(self.selected_item_index) {
            curr_item.map_item_to_editor()?;

            Ok(())
        } else {
            let msg = format!("couldn't get item for index: {}", self.selected_item_index);

            bail!("{}", msg);
        }
    }
}

#[derive(Debug, Display, Clone, Eq, PartialEq)]
pub enum ItemListTabType {
    #[strum(to_string = "Items")]
    Items,
    #[strum(to_string = "Lootlemon Items")]
    Lootlemon,
}

impl std::default::Default for ItemListTabType {
    fn default() -> Self {
        Self::Items
    }
}

#[derive(Debug)]
pub enum ItemEditorFileType<'a> {
    Save(&'a mut Bl3Save),
    ProfileBank(&'a mut Bl3Profile),
    // ProfileLostLoot(Bl3Profile),
}

#[derive(Debug, Clone)]
pub enum ItemEditorInteractionMessage {
    ItemPressed(usize),
    ItemsSearchInputChanged(String),
    ItemsLootLemonSearchInputChanged(String),
    ItemListReverseOrderPressed,
    ItemListItemTabPressed,
    ItemListLootlemonTabPressed,
    ItemListLootlemonImportPressed(usize),
    ItemListLootlemonOpenWebsitePressed(usize),
    ItemListLootlemonOpenWebsiteCompleted(MessageResult<()>),
    ShowAllAvailablePartsSelected(bool),
    AvailablePartsSearchInputChanged(String),
    AvailablePartsTabPressed,
    AvailableAnointmentsTabPressed,
    CurrentPartsSearchInputChanged(String),
    CurrentPartsTabPressed,
    CurrentAnointmentsTabPressed,
    ReorderCurrentPartsSelected(bool),
    ReorderCurrentPartsMoveUpPressed,
    ReorderCurrentPartsMoveDownPressed,
    ReorderCurrentPartsMoveTopPressed,
    ReorderCurrentPartsMoveBottomPressed,
    AvailablePartPressed(AvailablePartTypeIndex),
    AvailableAnointmentPressed(AvailablePartTypeIndex),
    CurrentPartPressed(bool, CurrentPartTypeIndex),
    CurrentAnointmentPressed(CurrentPartTypeIndex),
    ImportSerialInputChanged(String),
    CreateItemPressed,
    ImportItemFromSerialPressed,
    AllItemLevel(i32),
    SetAllItemLevelsPressed,
    ItemLevel(i32),
    DeleteItem(usize),
    DuplicateItem(usize),
    BalanceInputSelected(BalancePart),
    BalanceSearchInputChanged(String),
    InvDataInputSelected(InvDataPart),
    InvDataSearchInputChanged(String),
    ManufacturerSearchInputChanged(String),
    ManufacturerInputSelected(ManufacturerPart),
}

#[derive(Debug)]
pub struct ItemEditorInteractionResponse {
    pub notification: Option<Notification>,
    pub command: Option<Command<ItemEditorInteractionMessage>>,
}

impl ItemEditorInteractionMessage {
    pub fn update_state(
        self,
        item_editor_state: &mut ItemEditorState,
        item_editor_file_type: ItemEditorFileType,
    ) -> ItemEditorInteractionResponse {
        let mut notification = None;
        let mut command = None;

        match self {
            ItemEditorInteractionMessage::ItemPressed(item_index) => {
                item_editor_state.selected_item_index = item_index;

                item_editor_state
                    .map_current_item_if_exists(|i| {
                        i.editor.available_parts.part_type_index =
                            available_parts::AvailablePartTypeIndex {
                                category_index: 0,
                                part_index: 0,
                            }
                    })
                    .handle_ui_error("Failed to map selected item to editor", &mut notification);
            }
            ItemEditorInteractionMessage::ItemsSearchInputChanged(search_items_query) => {
                item_editor_state.search_items_input = search_items_query.to_lowercase();
            }
            ItemEditorInteractionMessage::ItemsLootLemonSearchInputChanged(
                search_lootlemon_items_query,
            ) => {
                item_editor_state.search_lootlemon_items_input =
                    search_lootlemon_items_query.to_lowercase();
            }
            ItemEditorInteractionMessage::ItemListReverseOrderPressed => {
                item_editor_state.item_list_is_reverse_order =
                    !item_editor_state.item_list_is_reverse_order;

                item_editor_state.items.reverse();

                // Maintain the selected item
                item_editor_state.selected_item_index =
                    item_editor_state.items.len() - item_editor_state.selected_item_index - 1;

                item_editor_state
                    .map_current_item_if_exists_to_editor_state()
                    .handle_ui_error(
                        "Failed to map previously selected item to editor when reversing order",
                        &mut notification,
                    );
            }
            ItemEditorInteractionMessage::ItemListItemTabPressed => {
                item_editor_state.search_items_input_state.focus();
                item_editor_state.item_list_tab_type = ItemListTabType::Items;
            }
            ItemEditorInteractionMessage::ItemListLootlemonTabPressed => {
                item_editor_state.search_lootlemon_items_input_state.focus();
                item_editor_state.item_list_tab_type = ItemListTabType::Lootlemon;
            }
            ItemEditorInteractionMessage::ItemListLootlemonImportPressed(id) => {
                if let Some(lootlemon_item) = item_editor_state.lootlemon_items.items.get(id) {
                    let item = lootlemon_item.item.clone();

                    match item_editor_state.item_list_is_reverse_order {
                        true => {
                            item_editor_state.insert_item(0, item);

                            item_editor_state.selected_item_index = 0;

                            item_editor_state.item_list_scrollable_state.snap_to(0.0);
                        }
                        false => {
                            item_editor_state.add_item(item);

                            item_editor_state.selected_item_index =
                                item_editor_state.items().len() - 1;

                            item_editor_state.item_list_scrollable_state.snap_to(1.0);
                        }
                    }

                    item_editor_state
                        .map_current_item_if_exists_to_editor_state()
                        .handle_ui_error(
                            "Failed to map Lootlemon item to editor",
                            &mut notification,
                        );

                    item_editor_state.search_lootlemon_items_input_state.focus();
                } else {
                    let msg = format!("Failed to import item from Lootlemon: couldn't find an item with index {}.", id);

                    error!("{}", msg);

                    notification = Some(Notification::new(msg, NotificationSentiment::Negative));
                }
            }
            ItemEditorInteractionMessage::ItemListLootlemonOpenWebsitePressed(id) => {
                if let Some(lootlemon_item) = item_editor_state.lootlemon_items.items.get(id) {
                    command = Some(Command::perform(
                        interaction::manage_save::item_editor::open_website(
                            lootlemon_item.link.clone(),
                        ),
                        |r| {
                            ItemEditorInteractionMessage::ItemListLootlemonOpenWebsiteCompleted(
                                MessageResult::handle_result(r),
                            )
                        },
                    ));
                } else {
                    let msg = format!(
                        "Failed to open Lootlemon Website: couldn't find an item with index {}.",
                        id
                    );

                    error!("{}", msg);

                    notification = Some(Notification::new(msg, NotificationSentiment::Negative));
                }
            }
            ItemEditorInteractionMessage::ItemListLootlemonOpenWebsiteCompleted(res) => {
                if let MessageResult::Error(e) = res {
                    let msg = format!("Failed to open Lootlemon Website: {}.", e);

                    error!("{}", msg);

                    notification = Some(Notification::new(msg, NotificationSentiment::Negative));
                }
            }
            ItemEditorInteractionMessage::ShowAllAvailablePartsSelected(selected) => {
                item_editor_state
                    .map_current_item_if_exists(|i| {
                        i.editor.available_parts.show_all_available_parts = selected;
                    })
                    .handle_ui_error(
                        "Failed to map item to editor when showing all available parts",
                        &mut notification,
                    );
            }
            ItemEditorInteractionMessage::AvailablePartsSearchInputChanged(search_input) => {
                item_editor_state
                    .map_current_item_if_exists(|i| {
                        i.editor.available_parts.search_input = search_input.to_lowercase();
                    })
                    .handle_ui_error(
                        "Failed to map item to editor when showing filtered available parts/anointments",
                        &mut notification,
                    );
            }
            ItemEditorInteractionMessage::AvailablePartsTabPressed => {
                item_editor_state
                    .map_current_item_if_exists(|i| {
                        i.editor.available_parts.scrollable_state.snap_to(0.0);
                        i.editor.available_parts.search_input = "".to_owned();
                        i.editor.available_parts.search_input_state.focus();
                        i.editor.available_parts.parts_tab_type = AvailablePartType::Parts;
                    })
                    .handle_ui_error(
                        "Failed to map item to editor when showing available parts",
                        &mut notification,
                    );
            }
            ItemEditorInteractionMessage::AvailableAnointmentsTabPressed => {
                item_editor_state
                    .map_current_item_if_exists(|i| {
                        i.editor.available_parts.scrollable_state.snap_to(0.0);
                        i.editor.available_parts.search_input = "".to_owned();
                        i.editor.available_parts.search_input_state.focus();
                        i.editor.available_parts.parts_tab_type = AvailablePartType::Anointments;
                    })
                    .handle_ui_error(
                        "Failed to map item to editor when showing available anointments",
                        &mut notification,
                    );
            }
            ItemEditorInteractionMessage::AvailablePartPressed(available_part_type_index) => {
                let selected_item_index = item_editor_state.selected_item_index;

                if let Some(current_item) =
                    item_editor_state.items_mut().get_mut(selected_item_index)
                {
                    if let Some(item_parts) = &mut current_item.item.item_parts {
                        if item_parts.parts().len() < MAX_BL3_ITEM_PARTS {
                            let part_selected = current_item
                                .editor
                                .available_parts
                                .parts
                                .get(available_part_type_index.category_index)
                                .and_then(|p| p.parts.get(available_part_type_index.part_index));

                            if let Some(part_selected) = part_selected {
                                let part_inv_key = &item_parts.part_inv_key;

                                if let Ok(bl3_part) = INVENTORY_SERIAL_DB
                                    .get_part_by_short_name(part_inv_key, &part_selected.part.name)
                                {
                                    if let Err(e) = current_item.item.add_part(bl3_part) {
                                        e.handle_ui_error(
                                            "Failed to add part to item",
                                            &mut notification,
                                        );
                                    } else {
                                        item_editor_state
                                            .map_current_item_if_exists(|i| {
                                                if i.editor.current_parts.reorder_parts {
                                                    i.editor
                                                        .current_parts
                                                        .scrollable_state
                                                        .snap_to(1.0);
                                                }

                                                i.editor.available_parts.part_type_index =
                                                    available_part_type_index
                                            })
                                            .handle_ui_error(
                                                "Failed to map item to editor after adding part to item",
                                                &mut notification,
                                            );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            ItemEditorInteractionMessage::AvailableAnointmentPressed(available_part_type_index) => {
                let selected_item_index = item_editor_state.selected_item_index;

                if let Some(current_item) =
                    item_editor_state.items_mut().get_mut(selected_item_index)
                {
                    if let Some(item_parts) = &mut current_item.item.item_parts {
                        if item_parts.generic_parts().len() < MAX_BL3_ITEM_ANOINTMENTS {
                            let anointment_selected = current_item
                                .editor
                                .available_parts
                                .parts
                                .get(available_part_type_index.category_index)
                                .and_then(|p| p.parts.get(available_part_type_index.part_index));

                            if let Some(anointment_selected) = anointment_selected {
                                if let Ok(bl3_part) = INVENTORY_SERIAL_DB.get_part_by_short_name(
                                    "InventoryGenericPartData",
                                    &anointment_selected.part.name,
                                ) {
                                    if let Err(e) = current_item.item.add_generic_part(bl3_part) {
                                        e.handle_ui_error(
                                            "Failed to add anointment to item",
                                            &mut notification,
                                        );
                                    } else {
                                        item_editor_state
                                            .map_current_item_if_exists(|i| {
                                                i.editor.available_parts.part_type_index =
                                                    available_part_type_index
                                            })
                                            .handle_ui_error(
                                                "Failed to map item to editor after adding anointment to item",
                                                &mut notification,
                                            );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            ItemEditorInteractionMessage::CurrentPartsSearchInputChanged(search_input) => {
                item_editor_state
                    .map_current_item_if_exists(|i| {
                        i.editor.current_parts.search_input = search_input.to_lowercase();
                    })
                    .handle_ui_error(
                        "Failed to map item to editor when showing filtered current parts/anointments",
                        &mut notification,
                    );
            }
            ItemEditorInteractionMessage::CurrentPartsTabPressed => {
                item_editor_state
                    .map_current_item_if_exists(|i| {
                        i.editor.current_parts.scrollable_state.snap_to(0.0);
                        i.editor.current_parts.search_input = "".to_owned();
                        i.editor.current_parts.search_input_state.focus();
                        i.editor.current_parts.reorder_parts = false;
                        i.editor.current_parts.parts_tab_type = CurrentPartType::Parts
                    })
                    .handle_ui_error("Failed to view current parts", &mut notification);
            }
            ItemEditorInteractionMessage::CurrentAnointmentsTabPressed => {
                item_editor_state
                    .map_current_item_if_exists(|i| {
                        i.editor.current_parts.scrollable_state.snap_to(0.0);
                        i.editor.current_parts.search_input = "".to_owned();
                        i.editor.current_parts.search_input_state.focus();
                        i.editor.current_parts.reorder_parts = false;
                        i.editor.current_parts.parts_tab_type = CurrentPartType::Anointments
                    })
                    .handle_ui_error("Failed to view current anointments", &mut notification);
            }
            ItemEditorInteractionMessage::ReorderCurrentPartsSelected(selected) => {
                item_editor_state
                    .map_current_item_if_exists(|i| {
                        i.editor.current_parts.part_type_index = CurrentPartTypeIndex::default();
                        i.editor.current_parts.reorder_parts = selected;

                        if selected {
                            i.editor.current_parts.scrollable_state.snap_to(0.0);
                        }
                    })
                    .handle_ui_error("Failed to reorder current parts", &mut notification);
            }
            ItemEditorInteractionMessage::ReorderCurrentPartsMoveUpPressed => {
                match item_editor_state.map_current_item_if_exists_result(|i| {
                    i.item
                        .move_part_up(&mut i.editor.current_parts.part_type_index.part_index)
                }) {
                    Ok(item) => item.editor.current_parts.search_input.clear(),
                    Err(e) => {
                        e.handle_ui_error("Failed to move selected part up", &mut notification)
                    }
                }
            }
            ItemEditorInteractionMessage::ReorderCurrentPartsMoveDownPressed => {
                match item_editor_state.map_current_item_if_exists_result(|i| {
                    i.item
                        .move_part_down(&mut i.editor.current_parts.part_type_index.part_index)
                }) {
                    Ok(item) => item.editor.current_parts.search_input.clear(),
                    Err(e) => {
                        e.handle_ui_error("Failed to move selected part down", &mut notification)
                    }
                }
            }
            ItemEditorInteractionMessage::ReorderCurrentPartsMoveTopPressed => {
                match item_editor_state.map_current_item_if_exists_result(|i| {
                    i.item
                        .move_part_top(&mut i.editor.current_parts.part_type_index.part_index)
                }) {
                    Ok(item) => {
                        item.editor.current_parts.search_input.clear();
                        item.editor.current_parts.scrollable_state.snap_to(0.0);
                    }
                    Err(e) => {
                        e.handle_ui_error("Failed to move selected part to top", &mut notification)
                    }
                }
            }
            ItemEditorInteractionMessage::ReorderCurrentPartsMoveBottomPressed => {
                match item_editor_state.map_current_item_if_exists_result(|i| {
                    i.item
                        .move_part_bottom(&mut i.editor.current_parts.part_type_index.part_index)
                }) {
                    Ok(item) => {
                        item.editor.current_parts.search_input.clear();
                        item.editor.current_parts.scrollable_state.snap_to(1.0);
                    }
                    Err(e) => e.handle_ui_error(
                        "Failed to move selected part to bottom",
                        &mut notification,
                    ),
                }
            }
            ItemEditorInteractionMessage::CurrentPartPressed(
                reorder_parts,
                current_part_type_index,
            ) => {
                let selected_item_index = item_editor_state.selected_item_index;

                if !reorder_parts {
                    if let Some(current_item) =
                        item_editor_state.items_mut().get_mut(selected_item_index)
                    {
                        let part_selected = current_item
                            .editor
                            .current_parts
                            .parts
                            .get(current_part_type_index.category_index)
                            .and_then(|p| p.parts.get(current_part_type_index.part_index));

                        if let Some(part_selected) = part_selected {
                            if let Err(e) = current_item.item.remove_part(&part_selected.part.part)
                            {
                                e.handle_ui_error(
                                    "Failed to remove part from item",
                                    &mut notification,
                                );
                            } else {
                                item_editor_state
                                    .map_current_item_if_exists_to_editor_state()
                                    .handle_ui_error(
                                        "Failed to map item with removed part to editor",
                                        &mut notification,
                                    );
                            }
                        }
                    }
                } else {
                    item_editor_state
                        .map_current_item_if_exists(|i| {
                            i.editor.current_parts.part_type_index = current_part_type_index;
                        })
                        .handle_ui_error("Failed to select part to reorder", &mut notification);
                }
            }
            ItemEditorInteractionMessage::CurrentAnointmentPressed(current_part_type_index) => {
                let selected_item_index = item_editor_state.selected_item_index;

                if let Some(current_item) =
                    item_editor_state.items_mut().get_mut(selected_item_index)
                {
                    let part_selected = current_item
                        .editor
                        .current_parts
                        .parts
                        .get(current_part_type_index.category_index)
                        .and_then(|p| p.parts.get(current_part_type_index.part_index));

                    if let Some(part_selected) = part_selected {
                        if let Err(e) = current_item
                            .item
                            .remove_generic_part(&part_selected.part.part)
                        {
                            e.handle_ui_error(
                                "Failed to remove anointment from item",
                                &mut notification,
                            );
                        } else {
                            item_editor_state
                                .map_current_item_if_exists_to_editor_state()
                                .handle_ui_error(
                                    "Failed to map item to editor after removing anointment from item",
                                    &mut notification,
                                );
                        }
                    }
                }
            }
            ItemEditorInteractionMessage::ImportSerialInputChanged(s) => {
                item_editor_state.import_serial_input = s;
            }
            ItemEditorInteractionMessage::CreateItemPressed => {
                let item = Bl3Item::from_serial_base64("BL3(BAAAAAD2aoA+P1vAEgA=)").unwrap();

                match item_editor_state.item_list_is_reverse_order {
                    true => {
                        item_editor_state.insert_item(0, item);

                        item_editor_state.selected_item_index = 0;

                        item_editor_state.item_list_scrollable_state.snap_to(0.0);
                    }
                    false => {
                        item_editor_state.add_item(item);

                        item_editor_state.selected_item_index = item_editor_state.items().len() - 1;

                        item_editor_state.item_list_scrollable_state.snap_to(1.0);
                    }
                }

                item_editor_state.search_items_input_state.focus();

                item_editor_state.item_list_tab_type = ItemListTabType::Items;

                item_editor_state
                    .map_current_item_if_exists_to_editor_state()
                    .handle_ui_error("Failed map created item to editor", &mut notification);
            }
            ItemEditorInteractionMessage::ImportItemFromSerialPressed => {
                let item_serial = item_editor_state.import_serial_input.trim();

                match Bl3Item::from_serial_base64(item_serial) {
                    Ok(item) => {
                        match item_editor_state.item_list_is_reverse_order {
                            true => {
                                item_editor_state.insert_item(0, item);

                                item_editor_state.selected_item_index = 0;

                                item_editor_state.item_list_scrollable_state.snap_to(0.0);
                            }
                            false => {
                                item_editor_state.add_item(item);

                                item_editor_state.selected_item_index =
                                    item_editor_state.items().len() - 1;

                                item_editor_state.item_list_scrollable_state.snap_to(1.0);
                            }
                        }

                        item_editor_state.search_items_input_state.focus();

                        item_editor_state.item_list_tab_type = ItemListTabType::Items;

                        item_editor_state
                            .map_current_item_if_exists_to_editor_state()
                            .handle_ui_error(
                                "Failed to map imported item to editor",
                                &mut notification,
                            );
                    }
                    Err(e) => {
                        e.handle_ui_error("Failed to import serial", &mut notification);
                    }
                }
            }
            ItemEditorInteractionMessage::AllItemLevel(item_level_input) => {
                item_editor_state.all_item_levels_input = item_level_input;
            }
            ItemEditorInteractionMessage::SetAllItemLevelsPressed => {
                let item_level = item_editor_state.all_item_levels_input as usize;

                let mut failed = false;

                for (i, item) in item_editor_state.items_mut().iter_mut().enumerate() {
                    if let Err(e) = item.item.set_level(item_level) {
                        let msg = format!("Failed to set level for item number: {} - {}", i, e);

                        e.handle_ui_error(&msg, &mut notification);

                        failed = true;

                        break;
                    }
                }

                if !failed {
                    item_editor_state
                        .map_current_item_if_exists_to_editor_state()
                        .handle_ui_error(
                            "Failed to map previously selected item to editor after updating all item levels",
                            &mut notification,
                        );
                }
            }
            ItemEditorInteractionMessage::ItemLevel(item_level_input) => {
                item_editor_state
                    .map_current_item_if_exists_result(|i| {
                        i.item.set_level(item_level_input as usize)
                    })
                    .handle_ui_error("Failed to set level for item", &mut notification);
            }
            ItemEditorInteractionMessage::DeleteItem(id) => {
                item_editor_state.remove_item(id);

                match item_editor_file_type {
                    ItemEditorFileType::Save(s) => s.character_data.remove_inventory_item(id),
                    ItemEditorFileType::ProfileBank(p) => p.profile_data.remove_bank_item(id),
                }

                if item_editor_state.selected_item_index != 0 {
                    item_editor_state.selected_item_index -= 1;
                }

                item_editor_state
                    .map_current_item_if_exists_to_editor_state()
                    .handle_ui_error(
                        "Failed to select an item to show in editor after deleting item",
                        &mut notification,
                    );
            }
            ItemEditorInteractionMessage::DuplicateItem(id) => {
                match item_editor_state.items.get(id) {
                    Some(item) => {
                        let item = item.item.clone();

                        match item_editor_state.item_list_is_reverse_order {
                            true => {
                                item_editor_state.insert_item(0, item);

                                item_editor_state.selected_item_index = 0;

                                item_editor_state.item_list_scrollable_state.snap_to(0.0);
                            }
                            false => {
                                item_editor_state.add_item(item);

                                item_editor_state.selected_item_index =
                                    item_editor_state.items().len() - 1;

                                item_editor_state.item_list_scrollable_state.snap_to(1.0);
                            }
                        }

                        item_editor_state.search_items_input_state.focus();

                        item_editor_state.item_list_tab_type = ItemListTabType::Items;

                        item_editor_state
                            .map_current_item_if_exists_to_editor_state()
                            .handle_ui_error(
                                "Failed to map duplicated item to editor",
                                &mut notification,
                            );
                    }
                    None => {
                        let msg = format!("Failed to duplicate item number {}: could not find this item to duplicate.", id);

                        notification =
                            Some(Notification::new(msg, NotificationSentiment::Negative));
                    }
                }
            }
            ItemEditorInteractionMessage::BalanceInputSelected(balance_selected) => {
                item_editor_state
                    .map_current_item_if_exists_result(|i| i.item.set_balance(balance_selected))
                    .handle_ui_error("Failed to set balance for item", &mut notification);
            }
            ItemEditorInteractionMessage::BalanceSearchInputChanged(balance_search_query) => {
                if balance_search_query.len() <= 500 {
                    item_editor_state
                        .map_current_item_if_exists(|i| {
                            i.editor.balance_search_input = balance_search_query.to_lowercase()
                        })
                        .handle_ui_error(
                            "Failed to set balance search field value for current item",
                            &mut notification,
                        );
                }
            }
            ItemEditorInteractionMessage::InvDataInputSelected(inv_data_selected) => {
                item_editor_state
                    .map_current_item_if_exists_result(|i| i.item.set_inv_data(inv_data_selected))
                    .handle_ui_error("Failed to set inventory data for item", &mut notification);
            }
            ItemEditorInteractionMessage::InvDataSearchInputChanged(inv_data_search_query) => {
                if inv_data_search_query.len() <= 500 {
                    item_editor_state
                        .map_current_item_if_exists(|i| {
                            i.editor.inv_data_search_input = inv_data_search_query.to_lowercase()
                        })
                        .handle_ui_error(
                            "Failed to set inventory data search field value for current item",
                            &mut notification,
                        );
                }
            }
            ItemEditorInteractionMessage::ManufacturerInputSelected(manufacturer_selected) => {
                item_editor_state
                    .map_current_item_if_exists_result(|i| {
                        i.item.set_manufacturer(manufacturer_selected)
                    })
                    .handle_ui_error("Failed to set manufacturer for item", &mut notification);
            }
            ItemEditorInteractionMessage::ManufacturerSearchInputChanged(
                manufacturer_search_query,
            ) => {
                if manufacturer_search_query.len() <= 500 {
                    item_editor_state
                        .map_current_item_if_exists(|i| {
                            i.editor.manufacturer_search_input =
                                manufacturer_search_query.to_lowercase()
                        })
                        .handle_ui_error(
                            "Failed to set manufacturer search field value for current item",
                            &mut notification,
                        );
                }
            }
        }

        ItemEditorInteractionResponse {
            notification,
            command,
        }
    }
}

pub fn view<F>(
    item_editor_state: &mut ItemEditorState,
    interaction_message: F,
) -> Container<Bl3Message>
where
    F: Fn(ItemEditorInteractionMessage) -> InteractionMessage + 'static + Copy,
{
    let selected_item_index = item_editor_state.selected_item_index;
    let number_of_items = item_editor_state.items.len();
    let number_of_lootlemon_items = item_editor_state.lootlemon_items.items.len();
    let item_list_tab_type = &item_editor_state.item_list_tab_type;

    let serial_importer = Row::new()
        .push(
            LabelledElement::create(
                "Import Serial",
                Length::Units(120),
                TextInputLimited::new(
                    &mut item_editor_state.import_serial_input_state,
                    "BL3(AwAAAABmboC7I9xAEzwShMJVX8nPYwsAAA==)",
                    &item_editor_state.import_serial_input,
                    500,
                    move |s| {
                        interaction_message(ItemEditorInteractionMessage::ImportSerialInputChanged(
                            s,
                        ))
                    },
                )
                .0
                .font(JETBRAINS_MONO)
                .padding(10)
                .size(17)
                .style(Bl3UiStyle)
                .into_element(),
            )
            .spacing(15)
            .width(Length::FillPortion(9))
            .align_items(Align::Center),
        )
        .push(
            Button::new(
                &mut item_editor_state.import_serial_button_state,
                Text::new("Import").font(JETBRAINS_MONO_BOLD).size(17),
            )
            .on_press(interaction_message(
                ItemEditorInteractionMessage::ImportItemFromSerialPressed,
            ))
            .padding(10)
            .style(Bl3UiStyle)
            .into_element(),
        )
        .align_items(Align::Center);

    let create_item_button = Container::new(
        Button::new(
            &mut item_editor_state.create_item_button_state,
            Text::new("Create Item").font(JETBRAINS_MONO_BOLD).size(17),
        )
        .on_press(interaction_message(
            ItemEditorInteractionMessage::CreateItemPressed,
        ))
        .padding(10)
        .style(Bl3UiStyle)
        .into_element(),
    );

    let edit_all_item_levels_input = Container::new(
        Row::new()
            .push(
                LabelledElement::create(
                    "All Levels",
                    Length::Units(95),
                    Tooltip::new(
                        NumberInput::new(
                            &mut item_editor_state.all_item_levels_input_state,
                            item_editor_state.all_item_levels_input,
                            1,
                            Some(MAX_CHARACTER_LEVEL as i32),
                            move |v| {
                                interaction_message(ItemEditorInteractionMessage::AllItemLevel(v))
                            },
                        )
                        .0
                        .font(JETBRAINS_MONO)
                        .padding(10)
                        .size(17)
                        .style(Bl3UiStyle)
                        .into_element(),
                        format!("Level must be between 1 and {}", MAX_CHARACTER_LEVEL),
                        tooltip::Position::Top,
                    )
                    .gap(10)
                    .padding(10)
                    .font(JETBRAINS_MONO)
                    .size(17)
                    .style(Bl3UiTooltipStyle),
                )
                .spacing(15)
                .width(Length::FillPortion(9))
                .align_items(Align::Center),
            )
            .push(
                Button::new(
                    &mut item_editor_state.all_item_levels_button_state,
                    Text::new("Set").font(JETBRAINS_MONO_BOLD).size(17),
                )
                .on_press(interaction_message(
                    ItemEditorInteractionMessage::SetAllItemLevelsPressed,
                ))
                .padding(10)
                .style(Bl3UiStyle)
                .into_element(),
            )
            .align_items(Align::Center),
    )
    .width(Length::Fill)
    .style(Bl3UiStyle);

    let general_options_row = Row::new()
        .push(create_item_button)
        .push(
            Container::new(serial_importer)
                .width(Length::FillPortion(8))
                .height(Length::Units(36))
                .style(Bl3UiStyle),
        )
        .push(
            Container::new(edit_all_item_levels_input)
                .width(Length::FillPortion(2))
                .height(Length::Units(36))
                .style(Bl3UiStyle),
        )
        .spacing(20);

    let mut item_editor = None;

    let search_items_query = match item_list_tab_type {
        ItemListTabType::Items => &item_editor_state.search_items_input,
        ItemListTabType::Lootlemon => &item_editor_state.search_lootlemon_items_input,
    };

    let filtered_items = get_filtered_items(
        search_items_query,
        &item_editor_state.item_list_tab_type,
        &item_editor_state.items,
        &item_editor_state.lootlemon_items.items,
    );

    let item_list_title_row = Row::new()
        .push(
            Container::new(tab_bar_button(
                &mut item_editor_state.item_list_items_tab_button_state,
                ItemListTabType::Items,
                &item_editor_state.item_list_tab_type,
                interaction_message(ItemEditorInteractionMessage::ItemListItemTabPressed),
                Some(format!("({})", number_of_items)),
            ))
            .padding(1)
            .width(Length::FillPortion(2)),
        )
        .push(
            Container::new(tab_bar_button(
                &mut item_editor_state.item_list_lootlemon_tab_button_state,
                ItemListTabType::Lootlemon,
                &item_editor_state.item_list_tab_type,
                interaction_message(ItemEditorInteractionMessage::ItemListLootlemonTabPressed),
                None,
            ))
            .padding(1)
            .width(Length::FillPortion(2)),
        )
        .align_items(Align::Center);

    let mut item_list_contents = Column::new()
        .push(Container::new(item_list_title_row))
        .width(Length::Fill);

    let item_list_search_input_placeholder = match item_editor_state.item_list_tab_type {
        ItemListTabType::Items => format!("Search {} items...", number_of_items),
        ItemListTabType::Lootlemon => format!("Search {} items...", number_of_lootlemon_items),
    };

    let item_list_search_input = match item_list_tab_type {
        ItemListTabType::Items => TextInputLimited::new(
            &mut item_editor_state.search_items_input_state,
            &item_list_search_input_placeholder,
            &item_editor_state.search_items_input,
            500,
            move |s| interaction_message(ItemEditorInteractionMessage::ItemsSearchInputChanged(s)),
        ),
        ItemListTabType::Lootlemon => TextInputLimited::new(
            &mut item_editor_state.search_lootlemon_items_input_state,
            &item_list_search_input_placeholder,
            &item_editor_state.search_lootlemon_items_input,
            500,
            move |s| {
                interaction_message(
                    ItemEditorInteractionMessage::ItemsLootLemonSearchInputChanged(s),
                )
            },
        ),
    };

    let mut item_list_search_row = Row::new()
        .push(
            item_list_search_input
                .0
                .font(JETBRAINS_MONO)
                .padding(10)
                .size(18)
                .style(Bl3UiStyle)
                .into_element(),
        )
        .align_items(Align::Center);

    let item_list_reverse_order_icon = match item_editor_state.item_list_is_reverse_order {
        true => svg::Handle::from_memory(ARROW_UP),
        false => svg::Handle::from_memory(ARROW_DOWN),
    };

    let reverse_order_button_tooltip_message = "Reverse the order of your items as they appear in this list (this does not modify the order in-game)";

    let item_list_reverse_order_button = Tooltip::new(
        Button::new(
            &mut item_editor_state.item_list_reverse_order_button_state,
            Svg::new(item_list_reverse_order_icon)
                .height(Length::Units(18))
                .width(Length::Units(18)),
        )
        .on_press(interaction_message(
            ItemEditorInteractionMessage::ItemListReverseOrderPressed,
        ))
        .padding(10)
        .style(Bl3UiStyle)
        .into_element(),
        reverse_order_button_tooltip_message,
        tooltip::Position::Top,
    )
    .gap(10)
    .padding(10)
    .font(JETBRAINS_MONO)
    .size(17)
    .style(Bl3UiTooltipStyle);

    // Keeping this here as we want the "editor" to show in both ItemListTabType views
    let inventory_items = item_editor_state.items.iter_mut().enumerate().fold(
        Column::new().align_items(Align::Start),
        |mut inventory_items, (i, item)| {
            let is_active = i == selected_item_index;

            let (list_item_button, curr_item_editor) = item.view(i, is_active, interaction_message);

            // Check if the curr item index is in our filtered_items to decide whether to show the
            // list item button or not.
            if item_list_tab_type == &ItemListTabType::Items
                && filtered_items.iter().any(|(fi_index, _)| *fi_index == i)
            {
                inventory_items = inventory_items.push(list_item_button);
            }

            if is_active {
                item_editor = curr_item_editor;
            }

            inventory_items
        },
    );

    match item_editor_state.item_list_tab_type {
        ItemListTabType::Items => {
            if number_of_items > 0 {
                item_list_search_row = item_list_search_row.push(item_list_reverse_order_button);
                item_list_contents = item_list_contents.push(item_list_search_row);

                if !filtered_items.is_empty() {
                    item_list_contents = item_list_contents.push(
                        Container::new(
                            Scrollable::new(&mut item_editor_state.item_list_scrollable_state)
                                .push(inventory_items)
                                .height(Length::Fill),
                        )
                        .padding(1),
                    );
                } else {
                    item_list_contents = item_list_contents.push(
                        Container::new(
                            Text::new(NO_SEARCH_RESULTS_FOUND_MESSAGE)
                                .font(JETBRAINS_MONO_BOLD)
                                .size(17)
                                .color(Color::from_rgb8(220, 220, 220)),
                        )
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(Align::Center)
                        .align_y(Align::Center),
                    );
                }
            } else {
                item_list_contents = item_list_contents.push(
                    Container::new(
                        Text::new("Please Import/Create an item to get started.")
                            .font(JETBRAINS_MONO_BOLD)
                            .size(17)
                            .color(Color::from_rgb8(220, 220, 220)),
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(Align::Center)
                    .align_y(Align::Center),
                );
            }
        }
        ItemListTabType::Lootlemon => {
            item_list_contents = item_list_contents.push(item_list_search_row);

            let mut view_index = 0;

            let lootlemon_items = item_editor_state
                .lootlemon_items
                .items
                .iter_mut()
                .enumerate()
                .fold(
                    Column::new().align_items(Align::Start),
                    |mut lootlemon_items, (i, item)| {
                        let lootlemon_item_view = item.view(view_index, interaction_message);

                        // Check if the curr item index is in our filtered_items to decide whether to show the
                        // list item button or not.
                        if filtered_items.iter().any(|(fi_index, _)| *fi_index == i) {
                            lootlemon_items = lootlemon_items.push(lootlemon_item_view);
                            view_index += 1;
                        }

                        lootlemon_items
                    },
                );

            if !filtered_items.is_empty() {
                item_list_contents = item_list_contents.push(
                    Container::new(
                        Scrollable::new(
                            &mut item_editor_state.item_list_lootlemon_scrollable_state,
                        )
                        .push(lootlemon_items)
                        .height(Length::Fill),
                    )
                    .padding(1),
                );
            } else {
                item_list_contents = item_list_contents.push(
                    Container::new(
                        Text::new(NO_SEARCH_RESULTS_FOUND_MESSAGE)
                            .font(JETBRAINS_MONO_BOLD)
                            .size(17)
                            .color(Color::from_rgb8(220, 220, 220)),
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(Align::Center)
                    .align_y(Align::Center),
                );
            }
        }
    };

    let item_list = Container::new(item_list_contents)
        .width(Length::FillPortion(3))
        .height(Length::Fill)
        .style(Bl3UiStyle);

    let mut item_list_and_editor = Row::new().push(item_list).spacing(20);

    if let Some(item_editor) = item_editor {
        item_list_and_editor = item_list_and_editor.push(
            item_editor
                .width(Length::FillPortion(7))
                .height(Length::Fill),
        );
    }

    let all_contents = Column::new()
        .push(general_options_row)
        .push(item_list_and_editor)
        .spacing(20);

    Container::new(all_contents).padding(30)
}

pub fn get_filtered_items(
    search_items_query: &str,
    item_list_tab_type: &ItemListTabType,
    items: &[ItemEditorListItem],
    lootlemon_items: &[ItemEditorLootlemonItem],
) -> Vec<(usize, Bl3Item)> {
    let filter_items = |item: &Bl3Item| -> bool {
        let search_items_query = search_items_query.trim();

        if search_items_query.is_empty() {
            return true;
        }

        // Handle this scenario explicitly as we want to search one if the other doesn't exist
        let balance_part_to_search = if let Some(name) = &item.balance_part().name {
            Some(name.to_lowercase())
        } else {
            item.balance_part()
                .short_ident
                .as_ref()
                .map(|short_ident| short_ident.to_lowercase())
        };

        balance_part_to_search
            .map(|n| n.contains(&search_items_query))
            .unwrap_or(false)
            || item
                .manufacturer_part()
                .short_ident
                .as_ref()
                .map(|mp| {
                    mp.to_title_case()
                        .to_lowercase()
                        .contains(&search_items_query)
                })
                .unwrap_or(false)
            || format!("level {}", item.level().to_string()).contains(&search_items_query)
            || item
                .item_parts
                .as_ref()
                .map(|ip| {
                    ip.item_type
                        .to_string()
                        .to_lowercase()
                        .contains(&search_items_query)
                        || ip
                            .rarity
                            .to_string()
                            .to_lowercase()
                            .contains(&search_items_query)
                        || ip
                            .weapon_type
                            .as_ref()
                            .map(|wt| wt.to_string().to_lowercase().contains(&search_items_query))
                            .unwrap_or(false)
                })
                .unwrap_or(false)
    };

    match item_list_tab_type {
        ItemListTabType::Items => items
            .par_iter()
            .enumerate()
            .map(|(i, item)| (i, &item.item))
            .filter(|(_, item)| filter_items(item))
            .map(|(i, item)| (i, item.clone()))
            .collect::<Vec<_>>(),
        ItemListTabType::Lootlemon => lootlemon_items
            .par_iter()
            .enumerate()
            .map(|(i, item)| (i, &item.item))
            .filter(|(_, item)| filter_items(item))
            .map(|(i, item)| (i, item.clone()))
            .collect::<Vec<_>>(),
    }
}
