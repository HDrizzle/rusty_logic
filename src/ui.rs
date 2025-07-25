use crate::{basic_components, prelude::*, resource_interface, simulator::{AncestryStack, Tool, SelectionState, GraphicSelectableItemRef}};
use eframe::{egui::{self, containers::Popup, scroll_area::ScrollBarVisibility, Align2, Button, DragValue, Frame, Painter, PopupCloseBehavior, Pos2, Rect, RectAlign, ScrollArea, Sense, Shape, Stroke, StrokeKind, Ui}, epaint::PathStroke};
use nalgebra::Transform2;
use serde::{Serialize, Deserialize};
use serde_json;
use std::{collections::HashSet, f32::consts::TAU, ops::RangeInclusive};

/// Style for the UI, loaded from /resources/styles.json
#[derive(Clone, Deserialize)]
pub struct Styles {
    /// Show a grid in the background
	pub show_grid: bool,
	/// Fraction of grid size that lines are drawn, 0.1 is probably good
	pub line_size_grid: f32,
	pub connection_dot_grid_size: f32,
    pub color_wire_floating: [u8; 3],
    pub color_wire_contested: [u8; 3],
    pub color_wire_low: [u8; 3],
    pub color_wire_high: [u8; 3],
    pub color_background: [u8; 3],
    pub color_foreground: [u8; 3],
    pub color_grid: [u8; 3],
	pub select_rect_color: [u8; 4],
	pub select_rect_edge_color: [u8; 3],
	pub color_wire_in_progress: [u8; 3]
}

impl Styles {
    pub fn load() -> Result<Self, String> {
        let raw_string: String = load_file_with_better_error(resource_interface::STYLES_FILE)?;
        let styles: Self = to_string_err(serde_json::from_str(&raw_string))?;
        Ok(styles)
    }
	pub fn color_from_logic_state(&self, state: LogicState) -> [u8; 3] {
		match state {
			LogicState::Driven(value) => match value {
				true => self.color_wire_high,
				false => self.color_wire_low
			},
			LogicState::Floating => self.color_wire_floating,
			LogicState::Contested => self.color_wire_contested
		}
	}
}

impl Default for Styles {
	fn default() -> Self {
		Self {
			show_grid: true,
			line_size_grid: 0.1,
			connection_dot_grid_size: 0.3,
			color_wire_floating: [128, 128, 128],
			color_wire_contested: [255, 0, 0],
			color_wire_low: [0, 0, 255],
			color_wire_high: [0, 255, 0],
			color_background: [0, 0, 0],
			color_foreground: [255, 255, 255],
			color_grid: [64, 64, 64],
			select_rect_color: [7, 252, 244, 128],
			select_rect_edge_color: [252, 7, 7],
			color_wire_in_progress: [2, 156, 99]
		}
	}
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UIData {
	pub selected: bool,
	pub position: IntV2,
	pub direction: FourWayDir,
	pub position_before_dragging: IntV2,
	pub dragging_offset: V2,
	pub local_bb: (V2, V2)
}

impl UIData {
	pub fn from_pos_dir(position: IntV2, direction: FourWayDir) -> Self {
		Self {
			position,
			direction,
			..Default::default()
		}
	}
	pub fn pos_to_parent_coords(&self, position: IntV2) -> IntV2 {
		self.direction.ro// TODO
	}
}

pub trait GraphicSelectableItem {
	/// The implementation if this is responsible for adding it's own position to the offset
	fn draw<'a>(&self, draw: &ComponentDrawInfo<'a>);
	fn get_ui_data(&self) -> &UIData;
	fn get_ui_data_mut(&mut self) -> &mut UIData;
	/// Used for the net highlight feature
	fn is_connected_to_net(&self, net_id: u64) -> bool;
	fn get_properties(&self) -> Vec<SelectProperty>;
	fn set_property(&mut self, property: SelectProperty);
	fn copy(&self) -> CopiedGraphicItem;
	/// Meant for external connections so that clickes can do something special instead of just selecting them
	/// Returns: Whether the click was "used", if so then it won't be selected normally but can still be command-clicked and included in a dragged rectangle
	fn accept_click(&mut self) -> bool {
		false
	}
	fn get_selected(&self) -> bool {
		self.get_ui_data().selected
	}
	fn set_selected(&mut self, selected: bool) {
		self.get_ui_data_mut().selected = selected;
	}
	/// Relative to Grid and self, return must be wrt global grid, hence why grid offset must be provided
	/// grid_offset will only correct for positions of nested sub-circuits, not the position of the object that it itself "knows about"
	fn bounding_box(&self, grid_offset: V2) -> (V2, V2) {
		let local_bb = self.get_ui_data().local_bb;
		(grid_offset + local_bb.0, grid_offset + local_bb.1)
	}
	/// Mouse is relative to Grid
	/// Things with complicated shapes should override this with something better
	fn is_click_hit(&self, mouse: V2, grid_offset: V2) -> bool {
		let bb: (V2, V2) = self.bounding_box(grid_offset);
		mouse.x >= bb.0.x && mouse.x <= bb.1.x && mouse.y >= bb.0.y && mouse.y <= bb.1.y
	}
	fn start_dragging(&mut self, current_mouse_pos: V2) {
		let current_pos = self.get_ui_data().position;
		self.get_ui_data_mut().dragging_offset = current_pos.to_v2() - current_mouse_pos;
	}
	fn dragging_to(&mut self, current_mouse_pos: V2) {
		let dragging_offset = self.get_ui_data().dragging_offset;
		self.get_ui_data_mut().position = round_v2_to_intv2(current_mouse_pos + dragging_offset);
	}
	fn stop_dragging(&mut self, final_mouse_pos: V2) {
		self.dragging_to(final_mouse_pos);
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CopiedGraphicItem {
	Component(EnumAllLogicDevices),
	Wire((IntV2, FourWayDir, u32)),
	ExternalConnection(LogicConnectionPin)
}

/// This is what will be serialized as JSON and put onto the clipboard
#[derive(Debug, Serialize, Deserialize)]
pub struct CopiedItemSet {
	pub items: Vec<CopiedGraphicItem>,
	/// So that everything will be shown at the same displacement wrt the mouse when paste is hit
	pub bb_center: IntV2
}

impl CopiedItemSet {
	pub fn new(
		items: Vec<CopiedGraphicItem>,
		bb_center: IntV2
	) -> Self {
		Self {
			items,
			bb_center
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectProperty {
	BitWidth(u32),
	PositionX(i32),
	PositionY(i32),
	GlobalConnectionState(Option<bool>),
	Direction(FourWayDir),
	DisplayCircuitAsBlock(bool),
	ClockEnabled(bool),
	ClockFreq(f32),
	ClockState(bool)
}

impl SelectProperty {
	pub fn ui_name(&self) -> String {
		match self {
			Self::BitWidth(_) => "Bit Width".to_owned(),
			Self::PositionX(_) => "X".to_owned(),
			Self::PositionY(_) => "Y".to_owned(),
			Self::GlobalConnectionState(_) => "I/O State".to_owned(),
			Self::Direction(_) => "Direction".to_owned(),
			Self::DisplayCircuitAsBlock(_) => "Display circuit as block".to_owned(),
			Self::ClockEnabled(_) => "Clock Enable".to_owned(),
			Self::ClockFreq(_) => "Clock Frequency".to_owned(),
			Self::ClockState(_) => "Clock State".to_owned()
		}
	}
	/// Add this property to a list on the UI
	/// Returns: Whether to update the property
	pub fn add_to_ui(&mut self, ui: &mut Ui) -> bool {
		match self {
			Self::BitWidth(n) => {
				ui.add(DragValue::new(n).range(RangeInclusive::new(1, 256)).clamp_existing_to_range(true)).changed()
			},
			Self::PositionX(x) => {
				if ui.button("<").clicked() {
					*x -= 1;
					return true;
				}
				if ui.add(DragValue::new(x)).changed() {
					return true;
				}
				if ui.button(">").clicked() {
					*x += 1;
					return true;
				}
				false
			},
			Self::PositionY(y) => {
				if ui.button("<").clicked() {
					*y -= 1;
					return true;
				}
				if ui.add(DragValue::new(y)).changed() {
					return true;
				}
				if ui.button(">").clicked() {
					*y += 1;
					return true;
				}
				false
			},
			Self::GlobalConnectionState(driven) => {
				let mut return_update = false;
				Popup::menu(&ui.button("I/O State")).align(RectAlign::RIGHT_START).show(|ui| {
					if ui.button("Driven (CLK)").clicked() {
						return_update = true;
					}
					if ui.button("Driven (0)").clicked() {
						*driven = Some(false);
						return_update = true;
					}
					if ui.button("Driven (1)").clicked() {
						*driven = Some(true);
						return_update = true;
					}
					if ui.button("Floating").clicked() {
						*driven = None;
						return_update = true;
					}
				});
				return_update
			},
			Self::Direction(dir) => {
				if ui.button("↶").clicked() {
					*dir = dir.turn_ccw();
					return true;
				}
				if ui.button("↷").clicked() {
					*dir = dir.turn_cw();
					return true;
				}
				return false;
			},
			Self::DisplayCircuitAsBlock(block) => {
				ui.checkbox(block, "Display circuit as block").changed()
			},
			Self::ClockEnabled(enable) => {
				ui.checkbox(enable, match enable {
					true => "Enabled",
					false => "Disabled"
				}).changed()
			},
			Self::ClockFreq(freq) => {
				ui.add(DragValue::new(freq).range(RangeInclusive::new(0.01, 1000000.0)).clamp_existing_to_range(false)).changed()
			},
			Self::ClockState(state) => {
				if ui.button(match state {
					true => "1",
					false => "0"
				}).clicked() {
					*state = !(*state);
					true
				}
				else {
					false
				}
			}
		}
	}
}

pub struct ComponentDrawInfo<'a> {
	/// Location of center of screen with respect to the grid, it is this way so that it will not have to adjusted when the grid is zoomed in/out
	pub screen_center_wrt_grid: V2,
	/// Pixels per grid increment
	pub grid_size: f32,
	pub painter: &'a Painter,
	/// Includes component's own position, 
	pub offset_grid: IntV2,
	pub direction: FourWayDir,
	pub styles: &'a Styles,
	pub rect_center: V2,
	rect_size_px: V2
}

impl<'a> ComponentDrawInfo<'a> {
	pub fn new(
		screen_center_wrt_grid: V2,
		grid_size: f32,
		painter: &'a Painter,
		offset_grid: IntV2,
		direction: FourWayDir,
		styles: &'a Styles,
		rect_center: V2,
		rect_size_px: V2
	) -> Self {
		Self {
			screen_center_wrt_grid,
			grid_size,
			painter,
			offset_grid,
			direction,
			styles,
			rect_center,
			rect_size_px
		}
	}
	pub fn draw_polyline(&self, points: Vec<V2>, stroke: [u8; 3]) {
		let px_points = points.iter().map(|p| self.grid_to_px(*p)).collect();
		self.painter.add(Shape::line(px_points, PathStroke::new(self.grid_size * self.styles.line_size_grid, u8_3_to_color32(stroke))));
	}
	pub fn draw_rect(&self, start_grid: V2, end_grid: V2, inside_stroke: [u8; 4], border_stroke: [u8; 3]) {
		let rectified_bb: (V2, V2) = merge_points_to_bb(vec![start_grid, end_grid]);
		let (start, end) = (self.grid_to_px(rectified_bb.0), self.grid_to_px(rectified_bb.1));
		self.painter.rect_filled(Rect{min: start, max: end}, 0, u8_4_to_color32(inside_stroke));
		self.painter.rect_stroke(Rect{min: start, max: end}, 0, Stroke::new(1.0, u8_3_to_color32(border_stroke)), StrokeKind::Outside);
	}
	pub fn draw_circle(&self, center: V2, radius: f32, stroke: [u8; 3]) {
		self.painter.circle_stroke(self.grid_to_px(center), radius * self.grid_size, Stroke::new(self.grid_size * self.styles.line_size_grid, u8_3_to_color32(stroke)));
	}
	pub fn draw_circle_filled(&self, center: V2, radius: f32, stroke: [u8; 3]) {
		self.painter.circle_filled(self.grid_to_px(center), radius * self.grid_size, u8_3_to_color32(stroke));
	}
	/// Egui doen't have an arc feature so I will use a polyline :(
	/// The end angle must be a larger number then the start angle
	pub fn draw_arc(&self, center_grid: V2, radius_grid: f32, start_deg: f32, end_deg: f32, stroke: [u8; 3]) {
		let mut polyline = Vec::<V2>::new();
		let seg_size: f32 = 5.0;
		let n_segs = ((end_deg - start_deg) / seg_size) as usize;
		for i in 0..n_segs+1 {
			let angle_rad = ((i as f32 * seg_size) + start_deg) * TAU / 360.0;
			polyline.push(V2::new(angle_rad.cos() * radius_grid, angle_rad.sin() * radius_grid) + center_grid);
		}
		self.draw_polyline(polyline, stroke);
	}
	pub fn grid_to_px(&self, grid: V2) -> egui::Pos2 {
		// TODO
		let nalgebra_v2 = ((self.direction.rotate_v2(grid) + self.offset_grid.to_v2()) * self.grid_size) + self.rect_center;
		if cfg!(feature = "reverse_y") {
			egui::Pos2{x: nalgebra_v2.x, y: self.rect_size_px.y - nalgebra_v2.y}
		} else {
			egui::Pos2{x: nalgebra_v2.x, y: nalgebra_v2.y}
		}
	}
	pub fn mouse_pos2_to_grid(&self, mouse_pos_y_backwards: Pos2) -> V2 {
		#[cfg(feature = "reverse_y")]
		let mouse_pos = V2::new(mouse_pos_y_backwards.x, self.rect_size_px.y - mouse_pos_y_backwards.y);
		#[cfg(not(feature = "reverse_y"))]
		let mouse_pos = emath_pos2_to_v2(mouse_pos_y_backwards);
		// TODO
		self.direction.rotate_v2_reverse(((mouse_pos - self.rect_center) / self.grid_size) - self.offset_grid.to_v2())
	}
	/// `offset_unrotated` is wrt parent coordinates, dir_ is the direction of the local coordinates of whatever this new drawer is being created for
	pub fn add_grid_pos_and_direction(&'a self, offset_unrotated: IntV2, dir_: FourWayDir) -> Self {
		let offset = self.direction.rotate_intv2(offset_unrotated);
		Self {
			screen_center_wrt_grid: self.screen_center_wrt_grid,
			grid_size: self.grid_size,
			painter: self.painter,
			offset_grid: self.offset_grid + offset,
			direction: self.direction.rotate_intv2(dir_.to_unit_int()).is_along_axis().unwrap(),
			styles: self.styles,
			rect_center: self.rect_center,
			rect_size_px: self.rect_size_px
		}
	}
}

pub struct LogicCircuitToplevelView {
	/// The top-level circuit of this view, its pins are rendered as interactive I/O pins
	circuit: LogicCircuit,
	/// Location of center of screen with respect to the grid, it is this way so that it will not have to adjusted when the grid is zoomed in/out
	screen_center_wrt_grid: V2,
	/// Pixels per grid increment
	grid_size: f32,
	logic_loop_error: bool,
	showing_component_popup: bool,
	component_search_text: String,
	all_logic_devices_search: Vec<EnumAllLogicDevices>,
	saved: bool,
	new_sub_cycles_entry: String,
	recompute_conns_nect_frame: bool
}

impl LogicCircuitToplevelView {
	pub fn new(circuit: LogicCircuit, saved: bool) -> Self {
		Self {
			circuit,
			screen_center_wrt_grid: V2::zeros(),
			grid_size: 15.0,
			logic_loop_error: false,
			showing_component_popup: false,
			component_search_text: String::new(),
			all_logic_devices_search: Vec::new(),
			saved,
			new_sub_cycles_entry: String::new(),
			recompute_conns_nect_frame: false
		}
	}
	pub fn draw(&mut self, ui: &mut Ui, styles: &Styles) {
		let inner_response = Frame::canvas(ui.style()).show(ui, |ui| {
			let mut propagate = true;// TODO: Change to false when rest of logic is implemented
			// TODO
			let canvas_size = ui.available_size_before_wrap();
			let (response, painter) = ui.allocate_painter(canvas_size, Sense::all());
			let draw_info = ComponentDrawInfo::new(
				self.screen_center_wrt_grid,
				self.grid_size,
				&painter,
				IntV2(0, 0),
				FourWayDir::default(),
				styles,
				emath_pos2_to_v2(response.rect.center()),
				emath_vec2_to_v2(canvas_size)
			);
			// First, detect user unput
			let input_state = ui.ctx().input(|i| i.clone());
			let recompute_connections: bool = self.circuit.toplevel_ui_interact(response, ui.ctx(), &draw_info, input_state);
			if recompute_connections {
				self.saved = false;
				self.circuit.check_wire_geometry_and_connections();
			}
			// Update
			if recompute_connections || propagate || self.recompute_conns_nect_frame {
				self.logic_loop_error = self.propagate_until_stable(PROPAGATION_LIMIT);
				self.recompute_conns_nect_frame = false;
			}
			// graphics help from https://github.com/emilk/egui/blob/main/crates/egui_demo_lib/src/demo/painting.rs
			// Draw circuit
			self.circuit.draw(&draw_info);
			// Right side toolbar
			self.circuit.tool.borrow().tool_select_ui(&draw_info);
		});
		// Top: general controls
		Popup::from_response(&inner_response.response).align(RectAlign{parent: Align2::LEFT_TOP, child: Align2::LEFT_TOP}).id("top-left controls".into()).show(|ui| {
			if ui.button("Save").clicked() {
				self.circuit.save_circuit().unwrap();
				self.saved = true;
			}
			if ui.button("+ Component / Subcircuit").clicked() {
				// Update component search list
				self.all_logic_devices_search = basic_components::list_all_basic_components();
				for file_path in resource_interface::list_all_circuit_files().unwrap() {
					self.all_logic_devices_search.push(EnumAllLogicDevices::SubCircuit(file_path, false, IntV2(0, 0), FourWayDir::default()));
				}
				self.showing_component_popup = true;
			}
			if self.circuit.tool.borrow().tool_select_allowed() {
				ui.menu_button("+ I/O Pin", |ui| {
					let mut new_pin_opt = Option::<LogicConnectionPin>::None;
					if ui.button("Input").clicked() {
						let mut new_pin = LogicConnectionPin::new(None, Some(LogicConnectionPinExternalSource::Global), IntV2(0, 0), FourWayDir::W, 1.0);
						new_pin.external_state = LogicState::Driven(false);
						new_pin_opt = Some(new_pin);
					}
					if ui.button("Output").clicked() {
						new_pin_opt = Some(LogicConnectionPin::new(None, Some(LogicConnectionPinExternalSource::Global), IntV2(0, 0), FourWayDir::E, 1.0));
					}
					if let Some(new_pin) = new_pin_opt {
						let handles: Vec<GraphicSelectableItemRef> = self.circuit.paste(&CopiedItemSet::new(vec![CopiedGraphicItem::ExternalConnection(new_pin)], IntV2(0, 0)));
						*self.circuit.tool.borrow_mut() = Tool::Select{
							selected_graphics: HashSet::from_iter(vec![handles[0].clone()].into_iter()),
							selected_graphics_state: SelectionState::FollowingMouse(V2::zeros())
						};
					}
				});
			}
			// Circuit settings (clock speed, etc)
			ui.collapsing("Circuit Settings", |ui| {
				ui.horizontal(|ui| {
					ui.label("Name");
					ui.text_edit_singleline(&mut self.circuit.generic_device.unique_name);
				});
				ui.horizontal(|ui| {
					ui.label("Sub compute cycles");
					ui.text_edit_singleline(&mut self.new_sub_cycles_entry);
					let button = Button::new("Update");
					match self.new_sub_cycles_entry.parse::<usize>() {
						Ok(sub_cycles) => {
							if ui.add(button).clicked() {
								self.circuit.generic_device.sub_compute_cycles = sub_cycles;
							}
						},
						Err(_) => {
							ui.add_enabled(false, button);
						}
					}
				});
			});
			// Active selection features
			if let Tool::Select{selected_graphics, selected_graphics_state: _} = &*self.circuit.tool.borrow() {
				if !selected_graphics.is_empty() {
					// Properties list
					ui.separator();
					ui.label("Properties");
					// All different variants of `SelectProperty`
					// Vec<(Property, whether they are all the same, Set of selected items to update when property is edited)>
					let mut unique_properties = Vec::<(SelectProperty, bool, HashSet<GraphicSelectableItemRef>)>::new();
					for graphic_handle in selected_graphics.iter() {
						let new_properties: Vec<SelectProperty> = self.circuit.run_function_on_graphic_item(graphic_handle.clone(), |item_box| item_box.get_properties());
						for property in new_properties {
							// Check if property enum variant is already included in `unique_properties`
							let mut variant_included = false;// Optional index of `unique_properties`
							for (prop_test, are_all_same, graphic_item_set) in unique_properties.iter_mut() {
								if prop_test.ui_name() == property.ui_name() {
									variant_included = true;
									if *prop_test != property {
										*are_all_same = false;
									}
									graphic_item_set.insert(graphic_handle.clone());
								}
							}
							if !variant_included {
								unique_properties.push((
									property,
									true,
									HashSet::from_iter(vec![graphic_handle.clone()].into_iter())
								));
							}
						}
					}
					// Display them
					for (prop, are_all_same, graphic_item_set) in unique_properties.iter_mut() {
						ui.horizontal(|ui| {
							ui.label(format!("{}:", prop.ui_name()));
							if prop.add_to_ui(ui) {
								self.saved = false;
								self.recompute_conns_nect_frame = true;
								for graphic_handle in graphic_item_set.iter() {
									self.circuit.run_function_on_graphic_item_mut(graphic_handle.clone(), |item_box| {item_box.set_property(prop.clone());});
								}
							}
						});
					}
				}
			}
		});
		if self.showing_component_popup {
			Popup::from_response(&inner_response.response).align(RectAlign{parent: Align2::CENTER_CENTER, child: Align2::CENTER_CENTER}).show(|ui| {
				ui.horizontal(|ui| {
					ui.label("Add Component / Sub-circuit");
					if ui.button("Cancel").clicked() {
						self.showing_component_popup = false;
					}
				});
				ui.text_edit_singleline(&mut self.component_search_text).request_focus();
				ScrollArea::vertical().show(ui, |ui| {
					for device_save in &self.all_logic_devices_search {
						if device_save.type_name().to_ascii_lowercase().contains(&self.component_search_text.to_ascii_lowercase()) {
							if ui.button(device_save.type_name()).clicked() {
								self.showing_component_popup = false;
								let handle = self.circuit.insert_component(device_save);
								*self.circuit.tool.borrow_mut() = Tool::Select{
									selected_graphics: HashSet::from_iter(vec![handle].into_iter()),
									selected_graphics_state: SelectionState::FollowingMouse(V2::zeros())
								};
							}
						}
					}
				});
			});
		}
	}
	/// Runs `compute_step()` repeatedly on the circuit until there are no changes, there must be a limit because there are circuits (ex. NOT gate connected to itself) where this would otherwise never end
	pub fn propagate_until_stable(&mut self, propagation_limit: usize) -> bool {
		let mut count: usize = 0;
		// TODO: keep track of state
		while count < propagation_limit {
			self.circuit.compute(&AncestryStack::new());
			count += 1;
		}
		return true;
	}
}


pub struct App {
	styles: Styles,
	circuit_tabs: Vec<LogicCircuitToplevelView>,
    current_tab_index: usize,
	show_new_circuit_popup: bool,
	new_circuit_name: String,
	new_circuit_path: String,
	load_circuit_err_opt: Option<String>
}

impl App {
	pub fn new() -> Self {
		// Load styles
		let styles: Styles = match Styles::load() {
			Ok(styles) => styles,
			Err(e) => {
				println!("Could not load styles: {}, resorting to default", e);
				Styles::default()
			}
		};
		Self {
			styles,
			circuit_tabs: Vec::new(),//vec![LogicCircuitToplevelView::new(create_simple_circuit(true), false)],
			current_tab_index: 0,
			show_new_circuit_popup: false,
			new_circuit_name: String::new(),
			new_circuit_path: String::new(),
			load_circuit_err_opt: None
		}
	}
}

impl eframe::App for App {
	fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
		let circuit_names: Vec<String> = self.circuit_tabs.iter().map(|toplevel| toplevel.circuit.generic_device.unique_name.clone()).collect();
		egui::CentralPanel::default().show(ctx, |ui: &mut Ui| {
			// This function by default is only run upon user interaction, so copied this from https://users.rust-lang.org/t/issues-while-writing-a-clock-with-egui/102752
			ui.ctx().request_repaint();
			ScrollArea::horizontal().scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
				ui.horizontal(|ui| {
					if ui.button("Home").clicked() {
						if self.current_tab_index != 0 {
							self.current_tab_index = 0;
						}
					}
					for (i, circuit_name) in circuit_names.iter().enumerate() {
						let name_for_ui: &str = match self.circuit_tabs[i].saved {
							true => circuit_name,
							false => &format!("{} *", circuit_name)
						};
						if ui.button(name_for_ui).clicked() {
							if i + 1 != self.current_tab_index {
								self.current_tab_index = i + 1;
							}
						}
					}
					let new_button_response = ui.button("+");
					if new_button_response.clicked() {
						self.new_circuit_name = String::new();
						self.new_circuit_path = String::new();
						self.load_circuit_err_opt = None;
					}
					Popup::menu(&new_button_response).close_behavior(PopupCloseBehavior::CloseOnClickOutside).align(RectAlign::RIGHT_START).align_alternatives(&[RectAlign::LEFT_START]).show(|ui| {
						ui.menu_button("New Circuit", |ui| {
							ui.horizontal(|ui| {
								ui.label("Name: ");
								ui.text_edit_singleline(&mut self.new_circuit_name);
							});
							ui.horizontal(|ui| {
								ui.label("Save file path: ");
								ui.text_edit_singleline(&mut self.new_circuit_path);
								ui.label(".json");
							});
							if ui.button("Create Circuit").clicked() {
								self.circuit_tabs.push(LogicCircuitToplevelView::new(LogicCircuit::new_mostly_default(self.new_circuit_name.clone(), self.new_circuit_path.clone()), false));
								self.current_tab_index = self.circuit_tabs.len();// Not an OBOE
							}
						});
						ui.menu_button("Load Circuit", |ui| {
							let files_list = resource_interface::list_all_circuit_files().unwrap();
							if files_list.len() == 0 {
								ui.label(format!("No circuit files found in {}", resource_interface::CIRCUITS_DIR));
							}
							ScrollArea::vertical().show(ui, |ui| {
								for file_path in files_list {
									if ui.selectable_label(false, &file_path).clicked() {
										match resource_interface::load_circuit(&file_path, false, true, IntV2(0, 0), FourWayDir::default()) {
											Ok(circuit) => {
												self.circuit_tabs.push(LogicCircuitToplevelView::new(circuit, true));
												self.current_tab_index = self.circuit_tabs.len();// Not an OBOE
											},
											Err(e) => {
												self.load_circuit_err_opt = Some(e);
											}
										}
									}
								}
							});
							if let Some(load_error) = &self.load_circuit_err_opt {
								ui.label(format!("Loading error: {}", load_error));
							}
						});
					});
				});
			});
			if self.current_tab_index == 0 {// Home tab
				ui.label("Rusty logic");
			}
			else {
				let circuit_toplevel: &mut LogicCircuitToplevelView = &mut self.circuit_tabs[self.current_tab_index - 1];
				circuit_toplevel.draw(ui, &self.styles);
			}
		});
	}
}