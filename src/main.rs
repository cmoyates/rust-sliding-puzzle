use std::{
    thread::sleep,
    time::{Duration, Instant},
};

use once_cell::sync::Lazy;
use rand::Rng;
use sfml::{
    graphics::{Color, Font, RenderTarget, RenderWindow, Text, Transformable},
    system::Vector2,
    window::{mouse, Event, Style, VideoMode},
    SfBox,
};

pub const FRAMERATE: u32 = 60;

pub static SCREEN_WIDTH: Lazy<u32> = Lazy::new(|| VideoMode::desktop_mode().width);
pub static SCREEN_HEIGHT: Lazy<u32> = Lazy::new(|| VideoMode::desktop_mode().height);

const FONT_DATA: &'static [u8] = include_bytes!("../assets/VT323-Regular.ttf");
pub static mut FONT: Lazy<SfBox<Font>> =
    Lazy::new(|| unsafe { Font::from_memory(FONT_DATA).unwrap() });

struct PuzzlePiece {
    pub window: RenderWindow,
    pub position: Vector2<f32>,
    pub target_position: Vector2<f32>,
    pub color: Color,
    pub target_color: Color,
}

impl PuzzlePiece {
    pub fn new(window: RenderWindow) -> Self {
        Self {
            window,
            position: Vector2::new(0.0, 0.0),
            target_position: Vector2::new(0.0, 0.0),
            color: Color::BLACK,
            target_color: Color::BLACK,
        }
    }

    pub fn set_position(&mut self, position: Vector2<f32>) {
        self.target_position = position;
    }

    pub fn set_color(&mut self, color: Color) {
        self.target_color = color;
    }

    pub fn update(&mut self) {
        self.position = lazy_smoothing_vector2(self.position, self.target_position, 0.1);
        self.window
            .set_position(Vector2::new(self.position.x as i32, self.position.y as i32));

        self.color = lazy_smoothing_color(self.color, self.target_color, 0.1);
    }
}

struct World {
    pub pieces: Vec<PuzzlePiece>,
    pub grabbed_piece: Option<usize>,
    pub grid: [[i8; 3]; 3],
    pub grab_offset: Vector2<i32>,
    pub available_move: Vector2<i8>,
    pub piece_size: u32,
    pub padding: u32,
    pub center: Vector2<u32>,
    pub playing: bool,
}

impl World {
    fn new(window_size: u32, padding: u32, mix_steps: u32) -> Self {
        let mut rng = rand::thread_rng();

        let mut pieces: Vec<PuzzlePiece> = Vec::new();

        let center = Vector2::new(
            *SCREEN_WIDTH / 2 - window_size / 2,
            *SCREEN_HEIGHT / 2 - window_size / 2,
        );

        for i in 0..8 {
            let mut window = RenderWindow::new(
                VideoMode::new(window_size, window_size, 32),
                &format!("{}", i + 1),
                Style::NONE,
                &Default::default(),
            );
            window.set_framerate_limit(FRAMERATE);

            pieces.push(PuzzlePiece::new(window));
        }

        // Make a 3x3 grid of ints
        let mut grid: [[i8; 3]; 3] = [[0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                let mut num: i8 = i * 3 + j;
                if num == 8 {
                    num = -1;
                }
                grid[i as usize][j as usize] = num;
            }
        }

        let mut last_swap = Vector2::new(0, 0);

        // Mix up the windows
        for _ in 0..mix_steps {
            let available_grid_pos = Self::m_get_grid_pos(grid, -1);

            // Get all adjacent positions
            let mut adjacent_positions: Vec<Vector2<i8>> = Vec::new();
            if available_grid_pos.x > 0 && last_swap.x != available_grid_pos.x - 1 {
                adjacent_positions
                    .push(Vector2::new(available_grid_pos.x - 1, available_grid_pos.y));
            }
            if available_grid_pos.x < 2 && last_swap.x != available_grid_pos.x + 1 {
                adjacent_positions
                    .push(Vector2::new(available_grid_pos.x + 1, available_grid_pos.y));
            }
            if available_grid_pos.y > 0 && last_swap.y != available_grid_pos.y - 1 {
                adjacent_positions
                    .push(Vector2::new(available_grid_pos.x, available_grid_pos.y - 1));
            }
            if available_grid_pos.y < 2 && last_swap.y != available_grid_pos.y + 1 {
                adjacent_positions
                    .push(Vector2::new(available_grid_pos.x, available_grid_pos.y + 1));
            }

            // Get a random adjacent position
            let random_index = rng.gen_range(0..adjacent_positions.len());

            // Swap the two positions
            let adjacent_position = adjacent_positions[random_index];
            let adjacent_index = grid[adjacent_position.y as usize][adjacent_position.x as usize];
            grid[adjacent_position.y as usize][adjacent_position.x as usize] = -1;
            grid[available_grid_pos.y as usize][available_grid_pos.x as usize] = adjacent_index;

            // Update the last swap
            last_swap = available_grid_pos;
        }

        // Set the positions of the windows
        for i in 0..8 {
            let grid_pos = Self::m_get_grid_pos(grid, i as i8);
            let grid_px = Self::m_grid_pos_to_px(
                window_size,
                padding,
                center,
                grid_pos.x as usize,
                grid_pos.y as usize,
            );

            let grid_px_f32 = Vector2::new(grid_px.x as f32, grid_px.y as f32);

            pieces[i].position = grid_px_f32;
            pieces[i].target_position = grid_px_f32;

            // TODO: Set the color of the window
            pieces[i].color = Color::BLACK;
            pieces[i].target_color = Color::BLACK;

            pieces[i].window.set_position(grid_px);
        }

        Self {
            pieces,
            grabbed_piece: None,
            grid,
            grab_offset: Vector2::new(0, 0),
            available_move: Vector2::new(0, 0),
            piece_size: window_size,
            padding,
            center,
            playing: true,
        }
    }

    pub fn s_update(&mut self) {
        for i in 0..8 {
            while let Some(event) = self.pieces[i].window.poll_event() {
                match event {
                    Event::MouseButtonPressed { button, x, y } => {
                        if button == mouse::Button::Left {
                            self.grabbed_piece = Some(i);
                            self.available_move = self.get_available_move(i);
                            if self.available_move.x != 0 || self.available_move.y != 0 {
                                self.grab_offset = Vector2::new(x, y);
                            }
                        }
                    }
                    Event::MouseButtonReleased { button, x: _, y: _ } => {
                        if button == mouse::Button::Left {
                            // If a window is grabbed
                            if let Some(grabbed_window) = self.grabbed_piece {
                                // If the window can move
                                if self.available_move.x != 0 || self.available_move.y != 0 {
                                    let current_grid_pos = self.get_grid_pos(grabbed_window);
                                    let current_grid_px = self.get_px_from_grid(grabbed_window);

                                    let available_grid_pos = Vector2::new(
                                        current_grid_pos.x + self.available_move.x,
                                        current_grid_pos.y + self.available_move.y,
                                    );
                                    let available_grid_px = self.grid_pos_to_px(
                                        available_grid_pos.x as usize,
                                        available_grid_pos.y as usize,
                                    );

                                    let window_position = self.pieces[i].position;
                                    let mut moved = false;

                                    // If the window can move horizontally
                                    if self.available_move.x != 0 {
                                        // If the window can move left
                                        if self.available_move.x > 0 {
                                            if window_position.x
                                                > current_grid_px.x as f32
                                                    + (self.padding / 2) as f32
                                                    + (self.piece_size / 2) as f32
                                            {
                                                self.grid[current_grid_pos.y as usize]
                                                    [current_grid_pos.x as usize] = -1;
                                                self.grid[available_grid_pos.y as usize]
                                                    [available_grid_pos.x as usize] =
                                                    grabbed_window as i8;

                                                moved = true;
                                            }
                                        } else {
                                            // If the window can move right
                                            if window_position.x
                                                < current_grid_px.x as f32
                                                    - (self.padding / 2) as f32
                                                    - (self.piece_size / 2) as f32
                                            {
                                                self.grid[current_grid_pos.y as usize]
                                                    [current_grid_pos.x as usize] = -1;
                                                self.grid[available_grid_pos.y as usize]
                                                    [available_grid_pos.x as usize] =
                                                    grabbed_window as i8;

                                                moved = true;
                                            }
                                        }
                                    }
                                    // If the window can move vertically
                                    else {
                                        // If the window can move up
                                        if self.available_move.y > 0 {
                                            if window_position.y
                                                > current_grid_px.y as f32
                                                    + (self.padding / 2) as f32
                                                    + (self.piece_size / 2) as f32
                                            {
                                                self.grid[current_grid_pos.y as usize]
                                                    [current_grid_pos.x as usize] = -1;
                                                self.grid[available_grid_pos.y as usize]
                                                    [available_grid_pos.x as usize] =
                                                    grabbed_window as i8;

                                                moved = true;
                                            }
                                        } else {
                                            // If the window can move down
                                            if window_position.y
                                                < current_grid_px.y as f32
                                                    - (self.padding / 2) as f32
                                                    - (self.piece_size / 2) as f32
                                            {
                                                self.grid[current_grid_pos.y as usize]
                                                    [current_grid_pos.x as usize] = -1;
                                                self.grid[available_grid_pos.y as usize]
                                                    [available_grid_pos.x as usize] =
                                                    grabbed_window as i8;

                                                moved = true;
                                            }
                                        }
                                    }

                                    // If the window didn't move reset its position
                                    if !moved {
                                        self.pieces[grabbed_window].set_position(Vector2::new(
                                            current_grid_px.x as f32,
                                            current_grid_px.y as f32,
                                        ));
                                    } else {
                                        self.pieces[grabbed_window].set_position(Vector2::new(
                                            available_grid_px.x as f32,
                                            available_grid_px.y as f32,
                                        ));
                                    }
                                }

                                // Reset the grabbed window
                                self.grabbed_piece = None;
                            }
                        }
                    }
                    _ => {}
                }
            }

            self.pieces[i].update();
        }

        // Grabbed window logic
        if let Some(grabbed_window) = self.grabbed_piece {
            // Get the current position of the grabbed window (grid and px)
            let current_grid_pos = self.get_grid_pos(grabbed_window);
            let current_grid_px = self.get_px_from_grid(grabbed_window);

            // Get the position of the available space (grid and px)
            let available_grid_pos = Vector2::new(
                current_grid_pos.x + self.available_move.x,
                current_grid_pos.y + self.available_move.y,
            );
            let available_grid_px =
                self.grid_pos_to_px(available_grid_pos.x as usize, available_grid_pos.y as usize);

            // Calculate the new position of the grabbed window
            let mouse_position = mouse::desktop_position();
            let new_x = if self.available_move.x != 0 {
                (mouse_position.x - self.grab_offset.x).clamp(
                    std::cmp::min(current_grid_px.x, available_grid_px.x),
                    std::cmp::max(current_grid_px.x, available_grid_px.x),
                )
            } else {
                current_grid_px.x
            };
            let new_y = if self.available_move.y != 0 {
                (mouse_position.y - self.grab_offset.y).clamp(
                    std::cmp::min(current_grid_px.y, available_grid_px.y),
                    std::cmp::max(current_grid_px.y, available_grid_px.y),
                )
            } else {
                current_grid_px.y
            };

            // Set the position
            self.pieces[grabbed_window].position = Vector2::new(new_x as f32, new_y as f32);
            self.pieces[grabbed_window].target_position = Vector2::new(new_x as f32, new_y as f32);
            self.pieces[grabbed_window]
                .window
                .set_position(Vector2::new(new_x, new_y));
        }

        // Check if the player won
        {
            let mut win = true;

            for i in 0..8 {
                let grid_pos = self.get_grid_pos(i);

                if grid_pos.y * 3 + grid_pos.x != i as i8 {
                    win = false;
                    break;
                }
            }

            if win {
                println!("You win!");
                self.playing = false;
            }
        }
    }

    pub fn s_render(&mut self) {
        for i in 0..8 {
            let grid_pos = self.get_grid_pos(i);

            let bg_color = if grid_pos.y * 3 + grid_pos.x == i as i8 {
                Color::rgb(0, 200, 0)
            } else {
                Color::rgb(200, 0, 0)
            };
            self.pieces[i].set_color(bg_color);

            let color = self.pieces[i].color;
            self.pieces[i].window.clear(color);

            // Write the window number in the middle of the window
            let mut text = Text::new(&format!("{}", i + 1), unsafe { &*FONT }, 100);
            text.set_fill_color(Color::WHITE);
            text.set_origin(Vector2::new(
                text.local_bounds().width / 2.0,
                text.local_bounds().height / 2.0,
            ));
            text.set_position(Vector2::new(42.5, 5.0));
            self.pieces[i].window.draw(&text);

            self.pieces[i].window.display();

            // Get the global mouse position
            let mouse_position = self.pieces[i].window.mouse_position();

            // Check if the mouse is in the window
            if mouse_position.x >= 0
                && mouse_position.x <= self.piece_size as i32
                && mouse_position.y >= 0
                && mouse_position.y <= self.piece_size as i32
            {
                self.pieces[i].window.request_focus();
            }
        }
    }

    pub fn get_available_move(&mut self, index: usize) -> Vector2<i8> {
        let grid_pos = self.get_grid_pos(index);

        // Check left
        if grid_pos.x > 0 {
            if self.grid[grid_pos.y as usize][(grid_pos.x - 1) as usize] == -1 {
                return Vector2::new(-1, 0);
            }
        }

        // Check right
        if grid_pos.x < 2 {
            if self.grid[grid_pos.y as usize][(grid_pos.x + 1) as usize] == -1 {
                return Vector2::new(1, 0);
            }
        }

        // Check up
        if grid_pos.y > 0 {
            if self.grid[(grid_pos.y - 1) as usize][grid_pos.x as usize] == -1 {
                return Vector2::new(0, -1);
            }
        }

        // Check down
        if grid_pos.y < 2 {
            if self.grid[(grid_pos.y + 1) as usize][grid_pos.x as usize] == -1 {
                return Vector2::new(0, 1);
            }
        }

        return Vector2::new(0, 0);
    }

    pub fn get_px_from_grid(&mut self, index: usize) -> Vector2<i32> {
        for x_index in 0..3 {
            for y_index in 0..3 {
                if self.grid[y_index][x_index] == index as i8 {
                    return self.grid_pos_to_px(x_index, y_index);
                }
            }
        }

        return Vector2::new(0, 0);
    }

    pub fn grid_pos_to_px(&mut self, x_index: usize, y_index: usize) -> Vector2<i32> {
        return Self::m_grid_pos_to_px(
            self.piece_size,
            self.padding,
            self.center,
            x_index,
            y_index,
        );
    }

    fn m_grid_pos_to_px(
        window_size: u32,
        padding: u32,
        center: Vector2<u32>,
        x_index: usize,
        y_index: usize,
    ) -> Vector2<i32> {
        let position = Vector2::new(
            (x_index as i32 - 1) * (window_size + padding) as i32 + center.x as i32,
            (y_index as i32 - 1) * (window_size + padding) as i32 + center.y as i32,
        );

        return position;
    }

    pub fn get_grid_pos(&mut self, index: usize) -> Vector2<i8> {
        return Self::m_get_grid_pos(self.grid, index as i8);
    }

    pub fn m_get_grid_pos(grid: [[i8; 3]; 3], index: i8) -> Vector2<i8> {
        for y_index in 0..3 {
            for x_index in 0..3 {
                if grid[y_index][x_index] == index as i8 {
                    return Vector2::new(x_index as i8, y_index as i8);
                }
            }
        }

        return Vector2::new(-1, -1);
    }
}

fn main() {
    let mut world = World::new(100, 10, 7);

    let mut last_update = Instant::now();
    let frame_duration = Duration::from_secs_f32(1.0 / FRAMERATE as f32);

    while world.playing {
        world.s_update();
        world.s_render();

        // Wait for next frame
        if let Some(sleep_duration) =
            (frame_duration).checked_sub(Instant::now().duration_since(last_update))
        {
            sleep(sleep_duration);
        }
        last_update = Instant::now();
    }
}

pub fn lazy_smoothing_vector2(
    current: Vector2<f32>,
    target: Vector2<f32>,
    threshold: f32,
) -> Vector2<f32> {
    Vector2::new(
        lazy_smoothing(current.x, target.x, threshold),
        lazy_smoothing(current.y, target.y, threshold),
    )
}

pub fn lazy_smoothing(current: f32, target: f32, threshold: f32) -> f32 {
    if (current - target).abs() < threshold {
        target
    } else {
        current + (target - current) * 0.15
    }
}

pub fn lazy_smoothing_color(current: Color, target: Color, threshold: f32) -> Color {
    Color::rgb(
        lazy_smoothing(current.r as f32, target.r as f32, threshold) as u8,
        lazy_smoothing(current.g as f32, target.g as f32, threshold) as u8,
        lazy_smoothing(current.b as f32, target.b as f32, threshold) as u8,
    )
}
