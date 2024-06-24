use crate::config::{ENTITIES_COLUMN_COUNT, ENTITIES_ROW_COUNT, ENTITIES_WIDTH, ENTITIES_HEIGHT};
use std::collections::HashMap;
use std::io::{ErrorKind, Read};
use std::mem::{size_of, transmute};
use std::os::unix::net::UnixStream;
use std::process::exit;
use std::thread::sleep;
use std::time;
use rand::Rng;
use crate::x11comm::x11_copy_area;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum EntityKind {
    Covered,
    Flagged,
    Uncovered0,
    Uncovered1,
    Uncovered2,
    Uncovered3,
    Uncovered4,
    Uncovered5,
    Uncovered6,
    Uncovered7,
    Uncovered8,
    MineExploded,
    MineIdle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SceneState {
    Uninitialized,
    Initializing,
    Ready,
    Won,
    Lost
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Position {
    x: u16,
    y: u16,
}

fn get_asset_coordinates() -> HashMap<EntityKind, Position> {
    let mut asset_coordinates = HashMap::new();
    asset_coordinates.insert(EntityKind::Uncovered0, Position { x: 0 * 16, y: 22 });
    asset_coordinates.insert(EntityKind::Uncovered1, Position { x: 1 * 16, y: 22 });
    asset_coordinates.insert(EntityKind::Uncovered2, Position { x: 2 * 16, y: 22 });
    asset_coordinates.insert(EntityKind::Uncovered3, Position { x: 3 * 16, y: 22 });
    asset_coordinates.insert(EntityKind::Uncovered4, Position { x: 4 * 16, y: 22 });
    asset_coordinates.insert(EntityKind::Uncovered5, Position { x: 5 * 16, y: 22 });
    asset_coordinates.insert(EntityKind::Uncovered6, Position { x: 6 * 16, y: 22 });
    asset_coordinates.insert(EntityKind::Uncovered7, Position { x: 7 * 16, y: 22 });
    asset_coordinates.insert(EntityKind::Uncovered8, Position { x: 8 * 16, y: 22 });
    asset_coordinates.insert(EntityKind::Covered, Position { x: 0, y: 38 });
    asset_coordinates.insert(EntityKind::Flagged, Position { x: 16, y: 38 });
    asset_coordinates.insert(EntityKind::MineExploded, Position { x: 32, y: 40 });
    asset_coordinates.insert(EntityKind::MineIdle, Position { x: 64, y: 40 });
    asset_coordinates
}

// Function to convert an index to row and column
fn idx_to_row_column(idx: u16) -> (u16, u16) {
    let row = idx / ENTITIES_COLUMN_COUNT;
    let column = idx % ENTITIES_COLUMN_COUNT;
    (row, column)
}

#[derive(Debug)]
pub(crate) struct Scene {
    state: SceneState,
    window_id: u32,
    gc_id: u32,
    sprite_pixmap_id: u32,
    displayed_entities: Vec<EntityKind>,
    mines: Vec<bool>,
}

impl Scene {
    pub(crate) fn new(window_id: u32, gc_id: u32, sprite_pixmap_id: u32) -> Self {
        return Scene{
            state: SceneState::Uninitialized,
            window_id,
            gc_id,
            sprite_pixmap_id,
            displayed_entities: vec![EntityKind::Covered; (ENTITIES_COLUMN_COUNT * ENTITIES_ROW_COUNT) as usize],
            mines: vec![false; (ENTITIES_COLUMN_COUNT * ENTITIES_ROW_COUNT) as usize],
        }
    }

    pub(crate) fn reset(&mut self)  {
        for entity in &mut self.displayed_entities {
            *entity = EntityKind::Covered;
        }

        let mut rng = rand::thread_rng();
        for mine in &mut self.mines {
            *mine = rng.gen_bool(0.1);
        }
    }

    pub fn render(&self, socket: &mut UnixStream) -> Result<(), std::io::Error> {
        let asset_coordinates = get_asset_coordinates();

        for (i, &entity) in self.displayed_entities.iter().enumerate() {
            if let Some(&pos) = asset_coordinates.get(&entity) {
                let (row, column) = idx_to_row_column(i as u16);
                x11_copy_area(
                    socket,
                    self.sprite_pixmap_id,
                    self.window_id,
                    self.gc_id,
                    pos.x,
                    pos.y,
                    column * ENTITIES_WIDTH,
                    row * ENTITIES_HEIGHT,
                    ENTITIES_WIDTH,
                    ENTITIES_HEIGHT,
                );
            }
        }
        Ok(())
    }

    pub fn wait_for_x11_events(&mut self, mut stream: UnixStream) -> Result<(), std::io::Error> {
        #[repr(C, packed)]
        struct GenericEvent {
            code: u8,
            pad: [u8; 31],
        }
        assert_eq!(size_of::<GenericEvent>(), 32);

        #[repr(C, packed)]
        struct KeyReleaseEvent {
            code: u8,
            detail: u8,
            sequence_number: u16,
            time: u32,
            root_id: u32,
            event: u32,
            child_id: u32,
            root_x: u16,
            root_y: u16,
            event_x: u16,
            event_y: u16,
            state: u16,
            same_screen: bool,
            pad1: u8,
        }
        assert_eq!(size_of::<KeyReleaseEvent>(), 32);

        #[repr(C, packed)]
        struct ButtonReleaseEvent {
            code: u8,
            detail: u8,
            seq_number: u16,
            timestamp: u32,
            root: u32,
            event: u32,
            child: u32,
            root_x: u16,
            root_y: u16,
            event_x: u16,
            event_y: u16,
            state: u16,
            same_screen: bool,
            pad1: u8,
        }
        assert_eq!(size_of::<ButtonReleaseEvent>(), 32);

        const EVENT_EXPOSURE: u8 = 0xc;
        const EVENT_KEY_RELEASE: u8 = 0x3;
        const EVENT_BUTTON_RELEASE: u8 = 0x5;

        const KEYCODE_ENTER: u8 = 36;

        loop {
            let mut generic_event = GenericEvent { code: 0, pad: [0; 31] };
            match stream.read_exact(unsafe {
                std::slice::from_raw_parts_mut(
                    &mut generic_event as *mut _ as *mut u8,
                    size_of::<GenericEvent>(),
                )
            }) {
                Ok(_) => {},
                Err(ref e) if e.kind() == ErrorKind::UnexpectedEof => {
                    println!("Connection closed");
                    return Ok(());
                },
                Err(e) => return Err(e),
            }

            match generic_event.code {
                EVENT_EXPOSURE => {
                    self.render(&mut stream)?;
                }
                EVENT_KEY_RELEASE => {
                    let event: KeyReleaseEvent = unsafe { transmute(generic_event) };
                    if event.detail == KEYCODE_ENTER {
                        self.reset();
                        self.render(&mut stream)?;
                    }
                }
                EVENT_BUTTON_RELEASE => {
                    let event: ButtonReleaseEvent = unsafe { transmute(generic_event) };
                    self.on_cell_clicked(event.event_x, event.event_y, event.detail);
                    self.render(&mut stream)?;
                }
                _ => {}
            }
        }
    }

    pub fn on_cell_clicked(&mut self, x: u16, y: u16, button: u8) {
        let (idx, row, column) = self.locate_entity_by_coordinate(x, y);

        match button {
            1 => { // Left click
                if self.displayed_entities[idx] == EntityKind::Flagged {
                    return; // Can't reveal flagged cells
                }

                let mined = self.mines[idx];

                if mined {
                    self.displayed_entities[idx] = EntityKind::MineExploded;
                    self.state = SceneState::Lost;
                    self.uncover_all_cells(EntityKind::MineExploded);
                } else {
                    self.uncover_cells_flood_fill(row, column);

                    if self.count_remaining_goals() == 0 {
                        self.state = SceneState::Won;
                        self.uncover_all_cells(EntityKind::MineIdle);
                    }
                }
            },
            3 => { // Right click
                if self.displayed_entities[idx] == EntityKind::Covered {
                    self.displayed_entities[idx] = EntityKind::Flagged;
                } else if self.displayed_entities[idx] == EntityKind::Flagged {
                    self.displayed_entities[idx] = EntityKind::Covered;
                }
            },
            _ => {} // Ignore other buttons
        }
    }

    fn uncover_cells_flood_fill(&mut self, row: usize, column: usize) {
        let i = self.row_column_to_idx(row as u16, column as u16) as usize;

        if self.mines[i] { return; }

        if self.displayed_entities[i] != EntityKind::Covered { return; }

        let mines_around_count = self.count_mines_around_cell(row, column);
        self.displayed_entities[i] = match mines_around_count {
            0 => EntityKind::Uncovered0,
            1 => EntityKind::Uncovered1,
            2 => EntityKind::Uncovered2,
            3 => EntityKind::Uncovered3,
            4 => EntityKind::Uncovered4,
            5 => EntityKind::Uncovered5,
            6 => EntityKind::Uncovered6,
            7 => EntityKind::Uncovered7,
            8 => EntityKind::Uncovered8,
            _ => panic!("Invalid mine count"),
        };

        // Only continue flood fill if this cell has no adjacent mines
        if mines_around_count == 0 {
            if row > 0 { self.uncover_cells_flood_fill(row - 1, column); }
            if column < (ENTITIES_COLUMN_COUNT - 1) as usize { self.uncover_cells_flood_fill(row, column + 1); }
            if row < (ENTITIES_ROW_COUNT - 1) as usize { self.uncover_cells_flood_fill(row + 1, column); }
            if column > 0 { self.uncover_cells_flood_fill(row, column - 1); }
            // Diagonal cells
            if row > 0 && column > 0 { self.uncover_cells_flood_fill(row - 1, column - 1); }
            if row > 0 && column < (ENTITIES_COLUMN_COUNT - 1) as usize { self.uncover_cells_flood_fill(row - 1, column + 1); }
            if row < (ENTITIES_ROW_COUNT - 1) as usize && column > 0 { self.uncover_cells_flood_fill(row + 1, column - 1); }
            if row < (ENTITIES_ROW_COUNT - 1) as usize && column < (ENTITIES_COLUMN_COUNT - 1) as usize { self.uncover_cells_flood_fill(row + 1, column + 1); }
        }
    }

    fn uncover_all_cells(&mut self, mine_type: EntityKind) {
        for i in 0..self.displayed_entities.len() {
            if self.mines[i] {
                self.displayed_entities[i] = mine_type;
            } else if self.displayed_entities[i] == EntityKind::Covered {
                let (row, column) = self.idx_to_row_column(i as u16);
                let mines_around_count = self.count_mines_around_cell(row as usize, column as usize);
                self.displayed_entities[i] = match mines_around_count {
                    0 => EntityKind::Uncovered0,
                    1 => EntityKind::Uncovered1,
                    2 => EntityKind::Uncovered2,
                    3 => EntityKind::Uncovered3,
                    4 => EntityKind::Uncovered4,
                    5 => EntityKind::Uncovered5,
                    6 => EntityKind::Uncovered6,
                    7 => EntityKind::Uncovered7,
                    8 => EntityKind::Uncovered8,
                    _ => panic!("Invalid mine count"),
                };
            }
        }
    }

    fn count_remaining_goals(&self) -> usize {
        self.displayed_entities.iter()
            .zip(self.mines.iter())
            .filter(|(&entity, &is_mine)| entity == EntityKind::Covered && !is_mine)
            .count()
    }

    fn count_mines_around_cell(&self, row: usize, column: usize) -> u8 {
        let mut count = 0;
        for i in -1..=1 {
            for j in -1..=1 {
                if i == 0 && j == 0 { continue; }
                let new_row = row as isize + i;
                let new_col = column as isize + j;
                if new_row >= 0 && new_row < ENTITIES_ROW_COUNT as isize &&
                   new_col >= 0 && new_col < ENTITIES_COLUMN_COUNT as isize {
                    let idx = self.row_column_to_idx(new_row as u16, new_col as u16) as usize;
                    if self.mines[idx] {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    fn idx_to_row_column(&self, idx: u16) -> (u16, u16) {
        let row = idx / ENTITIES_COLUMN_COUNT;
        let column = idx % ENTITIES_COLUMN_COUNT;
        (row, column)
    }

    fn row_column_to_idx(&self, row: u16, column: u16) -> u16 {
        row * ENTITIES_COLUMN_COUNT + column
    }

    fn locate_entity_by_coordinate(&self, win_x: u16, win_y: u16) -> (usize, usize, usize) {
        let column = win_x as usize / ENTITIES_WIDTH as usize;
        let row = win_y as usize / ENTITIES_HEIGHT as usize;
        let idx = self.row_column_to_idx(row as u16, column as u16);
        (idx as usize, row, column)
    }
}


