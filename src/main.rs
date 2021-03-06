use rltk::{Console, GameState, Point, Rltk};

use specs::{
    prelude::*,
    saveload::{SimpleMarker, SimpleMarkerAllocator},
};

pub mod camera;
pub mod map_builders;
pub mod raws;

mod ai;
mod components;
mod gamelog;
mod gamesystem;
mod gui;
mod map;
mod player;
mod random_table;
mod rect;
mod rex_assets;
mod spawner;
mod system_damage;
mod system_hunger;
mod system_inventory;
mod system_lighting;
mod system_map_indexing;
mod system_melee_combat;
mod system_particle;
mod system_saveload;
mod system_trigger;
mod system_visibility;

pub use components::*;
pub use gamelog::*;
pub use gamesystem::*;
pub use gui::*;
pub use map::*;
pub use player::*;
pub use random_table::*;
pub use rect::*;
pub use spawner::*;
pub use system_damage::*;
pub use system_hunger::*;
pub use system_inventory::*;
pub use system_lighting::*;
pub use system_map_indexing::*;
pub use system_melee_combat::*;
pub use system_particle::*;
pub use system_trigger::*;
pub use system_visibility::*;

#[macro_use]
extern crate lazy_static;

const TERMINAL_WIDTH: i32 = 80;
const TERMINAL_HEIGHT: i32 = 60;

const SHOW_MAPGEN_VISUALIZER: bool = false;
const SHOW_MAPGEN_VISUALIZER_COLOR: bool = false;
const MAPGEN_TIMER: f32 = 40.0;

#[macro_use]
extern crate specs_derive;

#[derive(PartialEq, Copy, Clone)]
pub enum RunState {
    AwaitingInput,
    PreRun,
    Ticking,
    ShowInventory,
    ShowDropItem,
    ShowTargeting {
        range: i32,
        item: Entity,
    },
    MainMenu {
        menu_selection: gui::MainMenuSelection,
    },
    SaveGame,
    NextLevel,
    PreviousLevel,
    ShowRemoveItem,
    GameOver,
    MagicMapReveal {
        row: i32,
    },
    MapGeneration,
    ShowCheatMenu,
}

pub struct State {
    pub ecs: World,
    mapgen_next_state: Option<RunState>,
    mapgen_history: Vec<Map>,
    mapgen_index: usize,
    mapgen_timer: f32,
}

impl State {
    fn run_systems(&mut self) {
        let mut mapindex = MapIndexingSystem {};
        mapindex.run_now(&self.ecs);
        let mut vis = VisibilitySystem {};
        vis.run_now(&self.ecs);
        let mut initiative = ai::InitiativeSystem {};
        initiative.run_now(&self.ecs);
        let mut turnstatus = ai::TurnStatusSystem {};
        turnstatus.run_now(&self.ecs);
        let mut quipper = ai::QuipSystem {};
        quipper.run_now(&self.ecs);
        let mut adjacent = ai::AdjacentAI {};
        adjacent.run_now(&self.ecs);
        let mut approach = ai::ApproachAI {};
        approach.run_now(&self.ecs);
        let mut flee = ai::FleeAI {};
        flee.run_now(&self.ecs);
        let mut visible = ai::VisibleAI {};
        visible.run_now(&self.ecs);
        let mut approach = ai::ApproachAI {};
        approach.run_now(&self.ecs);
        let mut defaultmove = ai::DefaultMoveAI {};
        defaultmove.run_now(&self.ecs);
        let mut triggers = TriggerSystem {};
        triggers.run_now(&self.ecs);
        let mut melee = MeleeCombatSystem {};
        melee.run_now(&self.ecs);
        let mut damage = DamageSystem {};
        damage.run_now(&self.ecs);
        let mut pickup = ItemCollectionSystem {};
        pickup.run_now(&self.ecs);
        let mut items = ItemUseSystem {};
        items.run_now(&self.ecs);
        let mut drop = ItemDropSystem {};
        drop.run_now(&self.ecs);
        let mut item_remove = ItemRemoveSystem {};
        item_remove.run_now(&self.ecs);
        let mut hunger = HungerSystem {};
        hunger.run_now(&self.ecs);
        let mut particles = ParticleSpawnSystem {};
        particles.run_now(&self.ecs);
        let mut lighting = LightingSystem {};
        lighting.run_now(&self.ecs);

        self.ecs.maintain();
    }
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut Rltk) {
        let mut newrunstate;
        {
            let runstate = self.ecs.fetch::<RunState>();
            newrunstate = *runstate;
        }
        ctx.cls(); //cls = Clear the screen
        system_particle::cull_dead_particles(&mut self.ecs, ctx);

        //Permet de ne pas créer la map temps que dans le menu principale
        match newrunstate {
            RunState::MainMenu { .. } => {}
            RunState::GameOver { .. } => {}
            _ => {
                camera::render_camera(&self.ecs, ctx);
                gui::draw_ui(&self.ecs, ctx);
            }
        }

        match newrunstate {
            RunState::ShowCheatMenu => {
                let result = gui::show_cheat_mode(self, ctx);
                match result {
                    gui::CheatMenuResult::Cancel => newrunstate = RunState::AwaitingInput,
                    gui::CheatMenuResult::NoResponse => {}
                    gui::CheatMenuResult::TeleportToExit => {
                        self.goto_level(1);
                        self.mapgen_next_state = Some(RunState::PreRun);
                        newrunstate = RunState::MapGeneration;
                    }
                }
            }
            RunState::MapGeneration => {
                if !SHOW_MAPGEN_VISUALIZER {
                    newrunstate = self.mapgen_next_state.unwrap();
                }
                ctx.cls();
                //draw_map(&self.mapgen_history[self.mapgen_index], ctx);
                if self.mapgen_index < self.mapgen_history.len() {
                    camera::render_debug_map(&self.mapgen_history[self.mapgen_index], ctx);
                }

                self.mapgen_timer += ctx.frame_time_ms;
                if self.mapgen_timer > MAPGEN_TIMER {
                    self.mapgen_timer = 0.0;
                    self.mapgen_index += 1;
                    if self.mapgen_index == self.mapgen_history.len() {
                        //loop {println!("loop");}
                        newrunstate = self.mapgen_next_state.unwrap();
                    }
                }
            }
            RunState::PreRun => {
                self.run_systems();
                newrunstate = RunState::AwaitingInput;
            }
            RunState::AwaitingInput => {
                newrunstate = player_input(self, ctx);
            }
            RunState::Ticking => {
                while newrunstate == RunState::Ticking {
                    self.run_systems();
                    self.ecs.maintain();
                    match *self.ecs.fetch::<RunState>() {
                        RunState::AwaitingInput => newrunstate = RunState::AwaitingInput,
                        RunState::MagicMapReveal { .. } => {
                            newrunstate = RunState::MagicMapReveal { row: 0 }
                        }
                        _ => newrunstate = RunState::Ticking,
                    }
                }
            }
            RunState::ShowInventory => {
                let result = gui::show_inventory(self, ctx);
                match result.0 {
                    gui::ItemMenuResult::Cancel => newrunstate = RunState::AwaitingInput,
                    gui::ItemMenuResult::NoResponse => {}
                    gui::ItemMenuResult::Selected => {
                        let item_entity = result.1.unwrap();
                        let is_ranged = self.ecs.read_storage::<Ranged>();
                        let is_item_ranged = is_ranged.get(item_entity);
                        if let Some(is_item_ranged) = is_item_ranged {
                            newrunstate = RunState::ShowTargeting {
                                range: is_item_ranged.range,
                                item: item_entity,
                            }
                        } else {
                            let mut intent = self.ecs.write_storage::<WantsToUseItem>();
                            intent
                                .insert(
                                    *self.ecs.fetch::<Entity>(),
                                    WantsToUseItem {
                                        item: item_entity,
                                        target: None,
                                    },
                                )
                                .expect("Unable to insert intent");
                            newrunstate = RunState::Ticking;
                        }
                    }
                }
            }
            RunState::ShowDropItem => {
                let result = gui::show_drop_menu(self, ctx);
                match result.0 {
                    gui::ItemMenuResult::Cancel => newrunstate = RunState::AwaitingInput,
                    gui::ItemMenuResult::NoResponse => {}
                    gui::ItemMenuResult::Selected => {
                        let item_entity = result.1.unwrap();
                        let mut intent = self.ecs.write_storage::<WantsToDropItem>();
                        intent
                            .insert(
                                *self.ecs.fetch::<Entity>(),
                                WantsToDropItem { item: item_entity },
                            )
                            .expect("Unable to insert intent");
                        newrunstate = RunState::Ticking;
                    }
                }
            }
            RunState::ShowTargeting { range, item } => {
                let result = gui::ranged_target(self, ctx, range);
                match result.0 {
                    gui::ItemMenuResult::Cancel => newrunstate = RunState::AwaitingInput,
                    gui::ItemMenuResult::NoResponse => {}
                    gui::ItemMenuResult::Selected => {
                        let mut intent = self.ecs.write_storage::<WantsToUseItem>();
                        intent
                            .insert(
                                *self.ecs.fetch::<Entity>(),
                                WantsToUseItem {
                                    item: item,
                                    target: result.1,
                                },
                            )
                            .expect("Unable to insert intent");
                        newrunstate = RunState::Ticking;
                    }
                }
            }
            RunState::MainMenu { .. } => {
                let result = gui::main_menu(self, ctx);
                match result {
                    gui::MainMenuResult::NoSelection { selected } => {
                        newrunstate = RunState::MainMenu {
                            menu_selection: selected,
                        }
                    }
                    gui::MainMenuResult::Selected { selected } => match selected {
                        gui::MainMenuSelection::NewGame => newrunstate = RunState::PreRun,
                        gui::MainMenuSelection::LoadGame => {
                            system_saveload::load_game(&mut self.ecs);
                            newrunstate = RunState::AwaitingInput;
                            system_saveload::delete_save();
                        }
                        gui::MainMenuSelection::Quit => ::std::process::exit(0),
                    },
                }
            }
            RunState::SaveGame => {
                system_saveload::save_game(&mut self.ecs);

                newrunstate = RunState::MainMenu {
                    menu_selection: gui::MainMenuSelection::LoadGame,
                };
            }
            RunState::NextLevel => {
                self.goto_level(1);
                self.mapgen_next_state = Some(RunState::PreRun);
                newrunstate = RunState::MapGeneration;
            }
            RunState::PreviousLevel => {
                self.goto_level(-1);
                self.mapgen_next_state = Some(RunState::PreRun);
                newrunstate = RunState::MapGeneration;
            }
            RunState::ShowRemoveItem => {
                let result = gui::remove_item_menu(self, ctx);
                match result.0 {
                    gui::ItemMenuResult::Cancel => newrunstate = RunState::AwaitingInput,
                    gui::ItemMenuResult::NoResponse => {}
                    gui::ItemMenuResult::Selected => {
                        let item_entity = result.1.unwrap();
                        let mut intent = self.ecs.write_storage::<WantsToRemoveItem>();
                        intent
                            .insert(
                                *self.ecs.fetch::<Entity>(),
                                WantsToRemoveItem { item: item_entity },
                            )
                            .expect("Unable to insert intent");
                        newrunstate = RunState::Ticking;
                    }
                }
            }
            RunState::GameOver => {
                self.game_over_cleanup();
                let result = gui::game_over(ctx);
                match result {
                    gui::GameOverResult::NoSelection => {}
                    gui::GameOverResult::QuitToMenu => {
                        //Moved above so there isn't map all over the place behing game over message
                        //self.game_over_cleanup();
                        newrunstate = RunState::MainMenu {
                            menu_selection: gui::MainMenuSelection::NewGame,
                        };
                    }
                }
            }
            RunState::MagicMapReveal { row } => {
                let mut map = self.ecs.fetch_mut::<Map>();
                for x in 0..map.width {
                    let idx = map.xy_idx(x as i32, row);
                    map.revealed_tiles[idx] = true;
                }
                if row == map.height - 2 {
                    newrunstate = RunState::Ticking;
                } else {
                    newrunstate = RunState::MagicMapReveal { row: row + 1 };
                }
            }
        }

        {
            let mut runwriter = self.ecs.write_resource::<RunState>();
            *runwriter = newrunstate;
        }
        system_damage::delete_the_dead(&mut self.ecs);
    }
}

impl State {
    fn generate_world_map(&mut self, new_depth: i32, offset: i32) {
        self.mapgen_index = 0;
        self.mapgen_timer = 0.0;
        self.mapgen_history.clear();
        let map_building_info = map::level_transition(&mut self.ecs, new_depth, offset);
        if let Some(history) = map_building_info {
            self.mapgen_history = history;
        } else {
            map::thaw_level_entities(&mut self.ecs);
        }
    }

    fn goto_level(&mut self, offset: i32) {
        freeze_level_entities(&mut self.ecs);

        // Build a new map and place the player
        let current_depth = self.ecs.fetch::<Map>().depth;
        self.generate_world_map(current_depth + offset, offset);

        // Notify the player
        let mut gamelog = self.ecs.fetch_mut::<gamelog::GameLog>();
        gamelog.entries.insert(0, "You change level.".to_string());
    }

    fn game_over_cleanup(&mut self) {
        //Delete everything
        let mut to_delete = Vec::new();
        for e in self.ecs.entities().join() {
            to_delete.push(e);
        }
        for del in to_delete.iter() {
            self.ecs.delete_entity(*del).expect("Deletion failed");
        }

        //spawn new player
        {
            let player_entity = spawner::player(&mut self.ecs, 0, 0);
            let mut player_entity_writer = self.ecs.write_resource::<Entity>();
            *player_entity_writer = player_entity;
        }

        // Replace the world maps
        self.ecs.insert(map::MasterDungeonMap::new());

        // Build a new map and place the player
        self.generate_world_map(1, 0);
    }
}

fn main() {
    use rltk::RltkBuilder;
    let context = RltkBuilder::simple(TERMINAL_WIDTH, TERMINAL_HEIGHT)
        .with_title("Roguelike Tutorial")
        .with_font("Taffer_10x10.png", 10, 10)
        .with_sparse_console(TERMINAL_WIDTH, TERMINAL_HEIGHT / 2, "Taffer_10x10.png")
        .build();

    //let mut context = Rltk::init_simple8x8(80, 50, "Hello Rust World", "resources");
    //context.with_post_scanlines(true);

    let mut gs = State {
        ecs: World::new(),
        mapgen_next_state: Some(RunState::MainMenu {
            menu_selection: gui::MainMenuSelection::NewGame,
        }),
        mapgen_index: 0,
        mapgen_history: Vec::new(),
        mapgen_timer: 0.0,
    };

    //Permet au systeme ECS de connaître les components disponible
    //Et crée systeme de stock pour chacun
    gs.ecs.register::<Position>();
    gs.ecs.register::<Renderable>();
    gs.ecs.register::<Player>();
    gs.ecs.register::<Viewshed>();
    gs.ecs.register::<Quips>();
    gs.ecs.register::<Name>();
    gs.ecs.register::<BlocksTile>();
    gs.ecs.register::<WantsToMelee>();
    gs.ecs.register::<SufferDamage>();
    gs.ecs.register::<Item>();
    gs.ecs.register::<ProvidesHealing>();
    gs.ecs.register::<InBackpack>();
    gs.ecs.register::<WantsToPickupItem>();
    gs.ecs.register::<WantsToUseItem>();
    gs.ecs.register::<WantsToDropItem>();
    gs.ecs.register::<Consumable>();
    gs.ecs.register::<Ranged>();
    gs.ecs.register::<InflictsDamage>();
    gs.ecs.register::<AreaOfEffect>();
    gs.ecs.register::<Targetable>();
    gs.ecs.register::<Confusion>();
    gs.ecs.register::<SimpleMarker<SerializeMe>>();
    gs.ecs.register::<DMSerializationHelper>();
    gs.ecs.register::<SerializationHelper>();
    gs.ecs.register::<Equippable>();
    gs.ecs.register::<Equipped>();
    gs.ecs.register::<MeleeWeapon>();
    gs.ecs.register::<Wearable>();
    gs.ecs.register::<WantsToRemoveItem>();
    gs.ecs.register::<ParticleLifetime>();
    gs.ecs.register::<HungerClock>();
    gs.ecs.register::<ProvidesFood>();
    gs.ecs.register::<MagicMapper>();
    gs.ecs.register::<Hidden>();
    gs.ecs.register::<EntryTrigger>();
    gs.ecs.register::<EntityMoved>();
    gs.ecs.register::<SingleActivation>();
    gs.ecs.register::<BlocksVisibility>();
    gs.ecs.register::<Door>();
    gs.ecs.register::<Attributes>();
    gs.ecs.register::<Skills>();
    gs.ecs.register::<Pools>();
    gs.ecs.register::<NaturalAttackDefense>();
    gs.ecs.register::<LootTable>();
    gs.ecs.register::<OtherLevelPosition>();
    gs.ecs.register::<LightSource>();
    gs.ecs.register::<Initiative>();
    gs.ecs.register::<MyTurn>();
    gs.ecs.register::<Faction>();
    gs.ecs.register::<WantsToApproach>();
    gs.ecs.register::<WantsToFlee>();
    gs.ecs.register::<MoveMode>();
    gs.ecs.register::<Chasing>();

    gs.ecs.insert(SimpleMarkerAllocator::<SerializeMe>::new());

    raws::load_raws();
    gs.ecs.insert(map::MasterDungeonMap::new());
    gs.ecs.insert(Map::new(1, 64, 64, "New Map"));
    gs.ecs.insert(Point::new(0, 0));
    gs.ecs.insert(rltk::RandomNumberGenerator::new());
    let player_entity = spawner::player(&mut gs.ecs, 0, 0);
    gs.ecs.insert(player_entity);
    gs.ecs.insert(RunState::MapGeneration);
    gs.ecs.insert(gamelog::GameLog {
        entries: vec!["Welcome to Rusty Roguelike".to_string()],
    });
    gs.ecs.insert(system_particle::ParticleBuilder::new());
    gs.ecs.insert(rex_assets::RexAssets::new());

    gs.generate_world_map(1, 0);

    rltk::main_loop(context, gs);
}
