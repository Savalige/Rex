use crossterm::event::poll;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::backend::Backend;
use ratatui::layout::Constraint;
use ratatui::style::Color;
use ratatui::Terminal;
use rusqlite::Connection;
use std::time::Duration;

use crate::activity_page::activity_ui;
use crate::activity_page::ActivityData;
use crate::add_tx_page::add_tx_ui;
use crate::chart_page::{chart_ui, ChartData};
use crate::home_page::home_ui;
use crate::home_page::TransactionData;
use crate::initial_page::initial_ui;
use crate::key_checker::{
    activity_keys, add_tx_keys, chart_keys, home_keys, initial_keys, search_keys, summary_keys,
    InputKeyHandler,
};
use crate::outputs::{HandlingOutput, UiHandlingError};
use crate::page_handler::{
    ActivityTab, ChartTab, CurrentUi, DateType, DeletionStatus, HomeTab, IndexedData, PopupState,
    SortingType, SummaryTab, TableData, TxTab,
};
use crate::popup_page::PopupData;
use crate::search_page::search_ui;
use crate::summary_page::{summary_ui, SummaryData};
use crate::tx_handler::TxData;
use crate::utility::get_all_tx_methods;

//const DEFAULT_BACKGROUND: Color = Color::Rgb(245, 245, 255);
//const DEFAULT_TEXT: Color = Color::Rgb(153, 78, 236);
//const DEFAULT_BOX: Color = Color::Rgb(255, 87, 51);
//const DEFAULT_SELECTED: Color = Color::Rgb(151, 251, 151);
//const DEFAULT_HIGHLIGHTED: Color = Color::Rgb(38, 38, 38);
//const DEFAULT_HEADER: Color = Color::Rgb(0, 150, 255);
//const DEFAULT_RED: Color = Color::Rgb(255, 51, 51);
//const DEFAULT_BLUE: Color = Color::Rgb(51, 51, 255);
//const DEFAULT_GRAY: Color = Color::Rgb(128, 128, 128);

const DRACULA_BACKGROUND: Color = Color::Rgb(40, 42, 54);
const DRACULA_TEXT: Color = Color::Rgb(248, 248, 242);
const DRACULA_CURRENT_LINE: Color = Color::Rgb(68, 71, 90);
const DRACULA_RED: Color = Color::Rgb(255, 85, 85);
const DRACULA_FOREGROUND: Color = Color::Rgb(248, 248, 242);
const DRACULA_YELLOW: Color = Color::Rgb(241, 250, 140);
//const DRACULA_ORANGE: Color = Color::Rgb(255, 184, 108);
const DRACULA_COMMENT: Color = Color::Rgb(98, 114, 164);

pub const BACKGROUND: Color = DRACULA_BACKGROUND;
pub const TEXT: Color = DRACULA_TEXT;
pub const BOX: Color = DRACULA_COMMENT;
pub const SELECTED: Color = DRACULA_COMMENT;
pub const HIGHLIGHTED: Color = DRACULA_CURRENT_LINE;
pub const HEADER: Color = DRACULA_COMMENT;
pub const RED: Color = DRACULA_RED;
pub const BLUE: Color = DRACULA_FOREGROUND;
pub const GRAY: Color = DRACULA_YELLOW;

/// Starts the interface and run the app
#[cfg(not(tarpaulin_include))]
pub fn start_app<B: Backend>(
    terminal: &mut Terminal<B>,
    new_version_data: &Option<Vec<String>>,
    conn: &mut Connection,
) -> Result<HandlingOutput, UiHandlingError> {
    // Setting up some default values. Let's go through all of them

    // contains the home page month list that is indexed
    let mut home_months = IndexedData::new_monthly();
    // contains the home page year list that is indexed
    let mut home_years = IndexedData::new_yearly();
    // contains the chart page month list that is indexed
    let mut chart_months = IndexedData::new_monthly();
    // contains the chart page year list that is indexed
    let mut chart_years = IndexedData::new_yearly();
    // contains the chart page mode selection list that is indexed
    let mut chart_modes = IndexedData::new_modes();
    // contains the chart page tx method selection list that is indexed
    let mut chart_tx_methods = IndexedData::new_tx_methods(conn);

    // contains the summary page month list that is indexed
    let mut summary_months = IndexedData::new_monthly();
    // contains the summary page year list that is indexed
    let mut summary_years = IndexedData::new_yearly();
    // contains the summary page mode selection list that is indexed
    let mut summary_modes = IndexedData::new_modes();
    // contains the Activity page month list that is indexed
    let mut activity_years = IndexedData::new_yearly();
    // contains the Activity page month list that is indexed
    let mut activity_months = IndexedData::new_monthly();

    // the selected widget on the Home Page. Default set to the month selection
    let mut home_tab = HomeTab::Months;

    // How summary table will be sorted
    let mut summary_sort = SortingType::ByTags;

    conn.execute("PRAGMA foreign_keys = ON", [])
        .expect("Could not enable foreign keys");

    // Stores all data relevant for home page such as balance, changes and txs
    let mut all_tx_data = TransactionData::new(home_months.index, home_years.index, conn);
    // Stores all activity for a specific month of a year alongside the txs involved in an activity
    let mut activity_data = ActivityData::new(activity_months.index, activity_years.index, conn);

    let mut search_txs = TransactionData::new_search(Vec::new(), Vec::new());
    // data for the Home Page's tx table
    let mut table = TableData::new(all_tx_data.get_txs());

    // The page which is currently selected. Default is the initial page
    let mut page = CurrentUi::Initial;
    // stores current popup status
    let mut popup_state = if let Some(data) = new_version_data {
        PopupState::NewUpdate(data.to_owned())
    } else {
        PopupState::Nothing
    };

    // Stores the current selected widget on Add Transaction page
    let mut add_tx_tab = TxTab::Nothing;
    // Store the current selected widget on Chart page
    let mut chart_tab = ChartTab::ModeSelection;
    // Store the current selected widget on Summary page
    let mut summary_tab = SummaryTab::ModeSelection;
    // Store the current selected widget on Search page
    let mut search_tab = TxTab::Nothing;
    // Store the current searching date type
    let mut search_date_type = DateType::Exact;
    // Store the current selected widget on Activity page
    let mut activity_tab = ActivityTab::Years;

    // Holds the data that will be/are inserted into the Add Tx page's input fields
    let mut add_tx_data = TxData::new();
    // Holds the data that will be/are inserted into the Summary Page
    let mut summary_data = SummaryData::new(conn);
    // Holds the data that will be/are inserted into the Search page's input fields
    let mut search_data = TxData::new_empty();
    // Holds the data that will be/are inserted into the Chart Page
    let mut chart_data = ChartData::new(conn);
    // Holds the popup data that will be/are inserted into the Popup page
    let mut popup_data = PopupData::new();

    // data for the Summary Page's table
    let mut summary_table = TableData::new(summary_data.get_table_data(
        &summary_modes,
        summary_months.index,
        summary_years.index,
    ));

    // data for the Search Page's table
    let mut search_table = TableData::new(Vec::new());

    // data for the Activity Page's table
    let mut activity_table = TableData::new(activity_data.get_txs());

    // the initial page REX loading index
    let mut starter_index = 0;

    // At what point the chart is current rendering to
    let mut chart_index: Option<f64> = None;

    // Whether the chart is in hidden mode
    let mut chart_hidden_mode = false;

    // Whether the summary is in hidden mode
    let mut summary_hidden_mode = false;

    // The initial popup when deleting tx will start on Yes value
    let mut deletion_status: DeletionStatus = DeletionStatus::Yes;

    // The current balance that is being shown on the home tab Balance column. Will change every loop util the actual balance is reached
    let mut balance_load = vec![0.0; get_all_tx_methods(conn).len() + 1];
    // The balance shown in the UI before the current actual balance that is being shown in the UI
    // If went from row 2 to row 3, this will contain the balance or row 2 to calculate the difference
    // we have to animate/load progressively
    let mut last_balance = Vec::new();
    // The actual current balance that is being shown
    let mut ongoing_balance = Vec::new();
    // If the difference between the ongoing and last balance is 100, each loop it adds/reduces x.xx% of the difference to the balance
    // till actual balance is reached. After each loop it gets increased by a little till 1.0 and key polling starts at 1.0, putting the app to sleep
    // A single var is used to keep track of load % for all values on the home page
    let mut load_percentage = 0.0;

    // Works similarly to balance load
    let mut changes_load = balance_load.clone();
    let mut last_changes = Vec::new();
    let mut ongoing_changes = Vec::new();

    let mut income_load = balance_load.clone();
    let mut last_income = Vec::new();
    let mut ongoing_income = Vec::new();

    let mut expense_load = balance_load.clone();
    let mut last_expense = Vec::new();
    let mut ongoing_expense = Vec::new();

    let mut daily_income_load = balance_load.clone();
    let mut daily_last_income = Vec::new();
    let mut daily_ongoing_income = Vec::new();

    let mut daily_expense_load = balance_load.clone();
    let mut daily_last_expense = Vec::new();
    let mut daily_ongoing_expense = Vec::new();

    // Contains whether in the chart whether a tx method is activated or not
    let mut chart_activated_methods = get_all_tx_methods(conn)
        .into_iter()
        .map(|s| (s, true))
        .collect();

    let mut popup_scroll_position = 0;
    let mut max_popup_scroll = 0;

    // Whether to reset home page stuff loading %
    // Will only turn true on initial run and when a key is pressed
    let mut to_reset = true;

    // Home and Add Tx Page balance data
    let mut balance_data = Vec::new();
    // Home and add tx page balance section's column space
    let mut width_data = Vec::new();
    let total_columns = get_all_tx_methods(conn).len() + 2;
    let width_percent = (100 / total_columns) as u16;

    // save the % of space each column should take in the Balance section based on the total
    // transaction methods/columns available
    for _ in 0..total_columns {
        width_data.push(Constraint::Percentage(width_percent));
    }

    // how it work:
    // Default value from above -> Goes to an interface page and render -> Wait for an event key press.
    //
    // If no keypress is detected in certain position it will start the next iteration -> interface -> Key check
    // Otherwise it will poll for keypress and locks the position
    //
    // If keypress is detected, send most of the &mut values to InputKeyHandler -> Gets mutated based on key press
    // -> loop ends -> start from beginning -> Send the new mutated values to the interface -> Keep up
    loop {
        // passing out relevant data to the ui function
        terminal
            .draw(|f| {
                match page {
                    CurrentUi::Home => home_ui(
                        f,
                        to_reset,
                        &home_months,
                        &home_years,
                        &mut table,
                        &mut balance_data,
                        &home_tab,
                        &mut width_data,
                        &mut balance_load,
                        &mut ongoing_balance,
                        &mut last_balance,
                        &mut changes_load,
                        &mut ongoing_changes,
                        &mut last_changes,
                        &mut income_load,
                        &mut ongoing_income,
                        &mut last_income,
                        &mut expense_load,
                        &mut ongoing_expense,
                        &mut last_expense,
                        &mut daily_income_load,
                        &mut daily_ongoing_income,
                        &mut daily_last_income,
                        &mut daily_expense_load,
                        &mut daily_ongoing_expense,
                        &mut daily_last_expense,
                        &mut load_percentage,
                        conn,
                    ),

                    CurrentUi::AddTx => add_tx_ui(
                        f,
                        to_reset,
                        &mut balance_data,
                        &add_tx_data,
                        &add_tx_tab,
                        &mut width_data,
                        &mut balance_load,
                        &mut ongoing_balance,
                        &mut last_balance,
                        &mut changes_load,
                        &mut ongoing_changes,
                        &mut last_changes,
                        &mut load_percentage,
                        conn,
                    ),

                    CurrentUi::Initial => initial_ui(f, starter_index),

                    CurrentUi::Chart => chart_ui(
                        f,
                        &chart_months,
                        &chart_years,
                        &chart_modes,
                        &chart_tx_methods,
                        &chart_data,
                        &chart_tab,
                        chart_hidden_mode,
                        &mut chart_index,
                        &chart_activated_methods,
                        conn,
                    ),

                    CurrentUi::Summary => summary_ui(
                        f,
                        &summary_months,
                        &summary_years,
                        &summary_modes,
                        &summary_data,
                        &mut summary_table,
                        &summary_tab,
                        summary_hidden_mode,
                        &summary_sort,
                        conn,
                    ),
                    CurrentUi::Search => search_ui(
                        f,
                        &search_data,
                        &search_tab,
                        &mut search_table,
                        &search_date_type,
                    ),
                    CurrentUi::Activity => activity_ui(
                        f,
                        &activity_months,
                        &activity_years,
                        &activity_tab,
                        &activity_data,
                        &mut activity_table,
                    ),
                }
                popup_data.create_popup(
                    f,
                    &popup_state,
                    &deletion_status,
                    popup_scroll_position,
                    &mut max_popup_scroll,
                );
            })
            .map_err(UiHandlingError::DrawingError)?;

        // Based on the UI status, either start polling for key press or continue the loop
        match page {
            CurrentUi::Initial => {
                // Initial page will loop indefinitely to animate the text
                if !poll(Duration::from_millis(40)).map_err(UiHandlingError::PollingError)? {
                    starter_index = (starter_index + 1) % 28;
                    continue;
                }
            }
            CurrentUi::Chart => {
                // If chart animation has ended, start polling
                if chart_index.is_some()
                    && !poll(Duration::from_millis(2)).map_err(UiHandlingError::PollingError)?
                {
                    continue;
                }
            }
            CurrentUi::Home | CurrentUi::AddTx => {
                // If balance loading hasn't ended yet, continue the loop
                if load_percentage < 1.0
                    && !poll(Duration::from_millis(2)).map_err(UiHandlingError::PollingError)?
                {
                    to_reset = false;
                    continue;
                }
                // Polling has started here. Unless a new key is pressed, it will never proceed further.
                // So after it's detected, we will reset the loading data on the home page
                to_reset = true;
            }

            _ => {}
        }

        // if not inside one of the duration polling, wait for keypress
        if let Event::Key(key) = event::read().map_err(UiHandlingError::PollingError)? {
            if key.kind != KeyEventKind::Press {
                to_reset = false;
                continue;
            }

            let mut handler = InputKeyHandler::new(
                key,
                &mut page,
                &mut balance_data,
                &mut popup_state,
                &mut add_tx_tab,
                &mut chart_tab,
                &mut summary_tab,
                &mut home_tab,
                &mut add_tx_data,
                &mut all_tx_data,
                &mut chart_data,
                &mut summary_data,
                &mut table,
                &mut summary_table,
                &mut home_months,
                &mut home_years,
                &mut chart_months,
                &mut chart_years,
                &mut chart_modes,
                &mut chart_tx_methods,
                &mut summary_months,
                &mut summary_years,
                &mut summary_modes,
                &mut summary_sort,
                &mut search_data,
                &mut search_date_type,
                &mut search_tab,
                &mut search_table,
                &mut search_txs,
                &mut activity_months,
                &mut activity_years,
                &mut activity_tab,
                &mut activity_data,
                &mut activity_table,
                &mut chart_index,
                &mut chart_hidden_mode,
                &mut summary_hidden_mode,
                &mut deletion_status,
                &mut ongoing_balance,
                &mut ongoing_changes,
                &mut ongoing_income,
                &mut ongoing_expense,
                &mut daily_ongoing_income,
                &mut daily_ongoing_expense,
                &mut chart_activated_methods,
                &mut popup_scroll_position,
                &mut max_popup_scroll,
                conn,
            );

            let status = match handler.page {
                CurrentUi::Initial => initial_keys(&mut handler),
                CurrentUi::Home => home_keys(&mut handler),
                CurrentUi::AddTx => add_tx_keys(&mut handler),
                CurrentUi::Chart => chart_keys(&mut handler),
                CurrentUi::Summary => summary_keys(&mut handler),
                CurrentUi::Search => search_keys(&mut handler),
                CurrentUi::Activity => activity_keys(&mut handler),
            };

            // If there is a status it means it needs to be handled outside the UI
            // Example quitting or J press for user inputs
            if let Some(output) = status {
                return Ok(output);
            }
        }
    }
}
