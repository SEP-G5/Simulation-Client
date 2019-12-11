use crate::rest;
use crate::transaction::Transaction;
use gtk::prelude::*;
use gtk::*;
use sourceview::*;
use std::collections::HashMap;
use std::{cell::RefCell, rc::Rc};

// ========================================================================== //

pub enum AppErr {
    GtkErr,
}

// ========================================================================== //

struct AppUI {
    /// URL input field
    url_input: Entry,
    /// Transaction list view
    list_view: TreeView,
    /// Transaction list model
    list_model: ListStore,
    /// JSON input area
    src_view: View,
}

pub struct AppData {
    /// List of transactions
    txs: HashMap<u32, Transaction>,
    /// Monotonically increasing ID
    id: u32,
}

pub struct App {
    /// Main application window
    window: Window,
    /// UI elements
    ui: Rc<RefCell<AppUI>>,
    /// Application data
    data: Rc<RefCell<AppData>>,
}

impl App {
    pub fn new(name: &str) -> Result<App, AppErr> {
        // Init GTK
        match gtk::init() {
            Ok(_) => {}
            Err(_) => return Err(AppErr::GtkErr),
        }

        // Create app window
        let window = Window::new(WindowType::Toplevel);
        window.set_title(name);
        window.set_default_size(640, 360);
        window.connect_delete_event(move |_, _| {
            gtk::main_quit();
            Inhibit(false)
        });

        // Create UI elements
        let url_input = EntryBuilder::new().build();
        url_input.set_text("http://localhost:8000/transaction");
        let list_view = TreeViewBuilder::new().headers_visible(true).build();
        let list_model = ListStore::new(&[u32::static_type(), String::static_type()]);
        list_view.set_model(Some(&list_model));
        let src_view = build_src_view("json");
        let ui = Rc::new(RefCell::new(AppUI {
            url_input,
            list_view,
            list_model,
            src_view,
        }));

        // Create app and build UI
        let data = Rc::new(RefCell::new(AppData {
            txs: HashMap::new(),
            id: 0,
        }));
        let mut app = App { window, ui, data };
        app.build_ui();
        Ok(app)
    }

    pub fn run(&self) {
        self.window.show_all();
        gtk::main();
    }

    fn build_ui(&mut self) {
        // Main VBox
        let vbox = Box::new(Orientation::Vertical, 0);
        self.window.add(&vbox);

        // Menu
        let menu = self.build_menu();
        vbox.add(&menu);

        // Pane
        let pane = PanedBuilder::new().border_width(3).expand(true).build();
        add_tree_column(&self.ui.borrow().list_view, "index", 0);
        add_tree_column(&self.ui.borrow().list_view, "id", 1);

        // List
        let wind = ScrolledWindowBuilder::new()
            .hscrollbar_policy(PolicyType::Automatic)
            .vscrollbar_policy(PolicyType::Automatic)
            .expand(true)
            .build();
        wind.add(&self.ui.borrow().list_view);
        pane.add(&wind);

        // Input area
        let input_area = self.build_input_area();
        pane.add(&input_area);
        vbox.add(&pane);

        // Setup list callback
        let ui_clone = self.ui.clone();
        let data_clone = self.data.clone();
        self.ui
            .borrow()
            .list_view
            .connect_button_press_event(move |_, _| {
                let selection = ui_clone.borrow().list_view.get_selection();
                if let Some((model, iter)) = selection.get_selected() {
                    let data = data_clone.borrow();
                    let idx = model.get_value(&iter, 0).get::<u32>().unwrap();
                    let tx = data.txs.get(&idx).unwrap();
                    let buffer = ui_clone.borrow_mut().src_view.get_buffer().unwrap();
                    buffer.set_text(&tx.to_json());
                }
                Inhibit(false)
            });

        let ui_clone = self.ui.clone();
        let data_clone = self.data.clone();
        self.ui.borrow().list_view.connect_cursor_changed(move |_| {
            let selection = ui_clone.borrow().list_view.get_selection();
            if let Some((model, iter)) = selection.get_selected() {
                let data = data_clone.borrow();
                let idx = model.get_value(&iter, 0).get::<u32>().unwrap();
                let tx = data.txs.get(&idx).unwrap();
                let buffer = ui_clone.borrow_mut().src_view.get_buffer().unwrap();
                buffer.set_text(&tx.to_json());
            }
        });
    }

    fn build_input_area(&mut self) -> Box {
        // Source field
        let wind = ScrolledWindowBuilder::new()
            .hscrollbar_policy(PolicyType::Automatic)
            .vscrollbar_policy(PolicyType::Automatic)
            .expand(true)
            .build();
        wind.add(&self.ui.borrow().src_view);

        // Buttons
        let hbox = Box::new(Orientation::Horizontal, 0);
        let send_btn = ButtonBuilder::new().label("Send").build();
        let ui_clone = self.ui.clone();
        let data_clone = self.data.clone();
        send_btn.connect_clicked(move |_| {
            let url = ui_clone.borrow().url_input.get_text().unwrap();
            let buffer = ui_clone.borrow_mut().src_view.get_buffer().unwrap();
            let json = buffer
                .get_text(&buffer.get_start_iter(), &buffer.get_end_iter(), true)
                .unwrap();
            let tx = Transaction::from_json(&json);
            insert_tx(&mut data_clone.borrow_mut(), &ui_clone.borrow(), tx);
            match rest::post(&url, &json) {
                Ok(_) => {}
                Err(e) => println!("Failed to send transaction ({})", e),
            }
        });
        let help_btn = ButtonBuilder::new().label("Help").build();
        hbox.add(&send_btn);
        hbox.add(&help_btn);

        let vbox = Box::new(Orientation::Vertical, 0);
        vbox.add(&wind);
        vbox.add(&self.ui.borrow().url_input);
        vbox.add(&hbox);
        vbox
    }

    fn build_menu(&mut self) -> MenuBar {
        let bar = MenuBar::new();

        // FILE
        let menu_file_item = MenuItem::new_with_mnemonic("_File");
        bar.append(&menu_file_item);
        let menu_file = Menu::new();
        menu_file_item.set_submenu(Some(&menu_file));

        // FILE - Quit
        let file_quit = MenuItem::new_with_label("Quit");
        file_quit.connect_activate(|_| {
            gtk::main_quit();
        });
        menu_file.append(&file_quit);

        // SIM
        let sim_menu_item = MenuItem::new_with_mnemonic("_Sim");
        bar.append(&sim_menu_item);
        let sim_menu = Menu::new();
        sim_menu_item.set_submenu(Some(&sim_menu));

        // SIM - New registry
        let sim_quit_btn = MenuItemBuilder::new().label("New Registry").build();
        //let list_clone = self.txs.clone();
        let ui_clone = self.ui.clone();
        sim_quit_btn.connect_activate(move |_| {
            let (tx, _) = Transaction::debug_make_register(format!("MY_BIKE"));
            let buffer = ui_clone.borrow_mut().src_view.get_buffer().unwrap();
            buffer.set_text(&tx.to_json());
            //list_clone.borrow_mut().push(format!("A tx"));
        });
        sim_menu.append(&sim_quit_btn);

        bar
    }
}

// ========================================================================== //

fn build_src_view(lang: &str) -> View {
    let lang_mgr = LanguageManager::new();
    let lang = lang_mgr
        .get_language(lang)
        .expect(&format!("Language support missing for {}", lang));
    let buf = Buffer::new_with_language(&lang);
    let view = View::new_with_buffer(&buf);
    view.set_show_line_numbers(true);
    view
}

fn add_tree_column(tree: &TreeView, title: &str, id: i32) {
    let column = TreeViewColumn::new();
    let cell = CellRendererText::new();
    column.pack_start(&cell, true);
    column.add_attribute(&cell, "text", id);
    column.set_title(title);
    tree.append_column(&column);
}

fn insert_tx(data: &mut AppData, ui: &AppUI, tx: Transaction) {
    let idx = data.id;
    println!("Inserting at index: {}", idx);
    data.id += 1;
    ui.list_model
        .insert_with_values(Some(0), &[0, 1], &[&idx, &tx.get_id()]);
    data.txs.insert(idx, tx);
}
