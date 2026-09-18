use super::*;
use abes_nice_things::Style;
use abes_nice_things::shred::ShredContents;
const EMPTY_KEY: (DropShred<[u8; 240]>, DropShred<[u8; 16]>) =
    (DropShred::new([0; 240]), DropShred::new([0; 16]));
pub fn tui(data: &mut Data) {
    assert_eq!(data.names.len(), data.entries.len());
    let mut state = TUIState {
        search: None,
        selected_entry: if data.names.len() == 0 {
            SelectedEntry::New
        } else {
            SelectedEntry::Entry(0)
        },
        selected_field: None,
    };
    state.render(data);
    while state.handle_input(data) {
        state.render(data)
    }
}

struct TUIState {
    search: Option<String>,
    selected_entry: SelectedEntry,
    selected_field: Option<(SelectedField, DropShred<Entry>)>,
}
impl TUIState {
    fn render(&self, data: &Data) {
        assert_eq!(data.names.len(), data.entries.len());
        let mut buf = Vec::new();
        // We clear the screen
        write!(buf, "\x1b[H\x1b[0J").unwrap();
        // The first line is the search/input
        write!(
            buf,
            "\x1b[H{}",
            self.search
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("/edit to edit")
        )
        .unwrap();
        let (width, height) = get_terminal_size();

        // Next is a divider between the search and the rest
        writeln!(buf, "\x1b[2;0H{}", "-".repeat(width)).unwrap();

        // The entries
        if !data.entries.is_empty() {
            // Now we have to figure out how many entries to show
            // -2 for search line
            // -2 for output line
            // -1 for new
            let visible_entries = (height - 5).min(data.names.len() + 1); // +1 for new
            let start = match self.selected_entry {
                SelectedEntry::Entry(index) => {
                    index.saturating_sub((visible_entries / 2).max(height - 5))
                }
                SelectedEntry::New => (data.names.len() - 1).saturating_sub(visible_entries),
            };
            for index in start..(start + visible_entries) {
                // Last thing: new button
                if index >= data.entries.len() {
                    let style = if let SelectedEntry::New = self.selected_entry {
                        *Style::new().yellow().background_red()
                    } else {
                        *Style::new().yellow()
                    };
                    writeln!(buf, "{style}new\x1b[0m").unwrap();
                    break;
                }
                if let SelectedEntry::Entry(entry) = self.selected_entry
                    && entry == index
                {
                    writeln!(
                        buf,
                        "{}{}\x1b[0m",
                        Style::new().background_red(),
                        data.names[index].as_str()
                    )
                    .unwrap();
                } else {
                    writeln!(buf, "{}", data.names[index].as_str()).unwrap();
                }
            }
        } else {
            writeln!(buf, "{}new\x1b[0m", Style::new().yellow().background_red()).unwrap();
        }

        // The fields
        if let Some((field, entry)) = &self.selected_field {
            // I'm just going to assume less fields than rows because I'm probably right an am lazy
            let horizontal = width / 2;
            // Password
            let password_style = if let SelectedField::Password = field {
                *Style::new().cyan().background_red()
            } else {
                *Style::new().cyan()
            };
            writeln!(buf, "\x1b[3;{horizontal}H{password_style}Password\x1b[0m").unwrap();

            // Fields
            for (current_index, (field_name, _)) in entry.fields.iter().enumerate() {
                if let SelectedField::Field(index) = field
                    && *index == current_index
                {
                    writeln!(
                        buf,
                        "\x1b[{horizontal}G{}{field_name}\x1b[0m",
                        Style::new().background_red()
                    )
                    .unwrap();
                } else {
                    writeln!(buf, "\x1b[{horizontal}G{field_name}").unwrap();
                }
            }

            // new
            let style = if let SelectedField::New = field {
                *Style::new().background_red().cyan()
            } else {
                *Style::new().cyan()
            };
            writeln!(buf, "\x1b[{horizontal}G{style}new\x1b[0m").unwrap();
        }

        // Putting the cursor where it should be
        write!(
            buf,
            "\x1b[0;{}H",
            self.search
                .as_ref()
                .map(|search| search.len() + 1)
                .unwrap_or(0)
        )
        .unwrap();

        // Sending it off to the screen
        std::io::stdout().write_all(buf.as_slice()).unwrap();
        std::io::stdout().flush().unwrap();
    }
    // returns if it should continue
    fn handle_input(&mut self, data: &mut Data) -> bool {
        weirdify();
        let mut stdin = std::io::stdin();
        let mut buf = [0];
        loop {
            stdin.read_exact(&mut buf).unwrap();
            match buf[0] {
                // Contol sequence starter
                0x1B => {
                    stdin.read_exact(&mut buf).unwrap();
                    stdin.read_exact(&mut buf).unwrap();
                    match buf[0] {
                        // up
                        b'A' => {
                            if let Some((selected_field, entry)) = &mut self.selected_field {
                                selected_field.decriment(entry.fields.len());
                            } else {
                                self.selected_entry.decriment(data.entries.len());
                            }
                        }
                        // down
                        b'B' => {
                            if let Some((selected_field, entry)) = &mut self.selected_field {
                                selected_field.increment(entry.fields.len());
                            } else {
                                self.selected_entry.increment(data.entries.len());
                            }
                        }
                        // right
                        b'C' => {
                            select(self, data);
                        }
                        // left
                        b'D' => self.selected_field = None,
                        _ => continue,
                    }
                }
                b'\n' => {
                    select(self, data);
                }
                // delete
                0x7F => {
                    if let Some(search) = &mut self.search {
                        search.pop().unwrap();
                        if search.is_empty() {
                            self.search = None;
                        }
                    }
                }
                other => {
                    if other.is_ascii_graphic() {
                        if self.search.is_none() {
                            self.search = Some(String::new());
                        }
                        self.search.as_mut().unwrap().push(other as char);
                    } else {
                        continue;
                    }
                }
            }
            break;
        }
        return true;
        fn select(state: &mut TUIState, data: &mut Data) -> bool {
            if let Some((selected_field, entry)) = &mut state.selected_field {
                if let SelectedField::New = selected_field {
                    // First we get the name
                    let name = input("Field name");
                    if name.is_empty() {
                        return true;
                    }

                    // Then the value
                    print!(
                        "\x1b[H{}Field value:\x1b[0m ",
                        Style::new().background_yellow()
                    );
                    let mut value = DropShred::new(String::new());
                    std::io::stdout().flush().unwrap();
                    if !input_hidden(&mut value) {
                        return true;
                    }

                    // Now we update everything
                    entry.fields.push((name, (*value).clone()));
                    let (mut key, mut iv) = (DropShred::new([0; 240]), DropShred::new([0; 16]));
                    if !get_key(&mut key, &mut iv, data.password_hash) {
                        return true;
                    }
                    let mut entry_iv = iv.clone();
                    get_entry_iv(state.selected_entry.unwrap_entry(), &mut entry_iv);
                    let mut encrypted = Vec::new();
                    let mut encrypter =
                        EncryptWriter::new(&mut encrypted, *entry_iv, key.as_slice(), FLUSH_SOURCE)
                            .unwrap();
                    entry.to_binary(&mut encrypter).unwrap();
                    std::mem::drop(encrypter);
                    data.entries[state.selected_entry.unwrap_entry()] = encrypted;
                    data.save(&key, &iv);
                    data.decrypt_entry(state.selected_entry.unwrap_entry(), &key, &entry_iv, entry);
                } else if state.search.as_ref().map(|s| s.as_str()) == Some("/edit") {
                    state.search = None;
                    // Editing a field will edit the name or value
                    match input("name or value").to_lowercase().as_str() {
                        "name" | "n" => {
                            // Can't change the name of the password
                            if let SelectedField::Password = selected_field {
                                return true;
                            }
                            let new_name = input("name");
                            if new_name.is_empty() {
                                return true;
                            }
                            entry.fields[selected_field.unwrap_field()].0 = new_name;
                        }
                        "value" | "val" | "v" => {
                            let mut new_value = DropShred::new(String::new());
                            if !input_hidden_prompt("New value", &mut new_value) {
                                return true;
                            }
                            if new_value.is_empty() {
                                return true;
                            }
                            match selected_field {
                                SelectedField::Password => entry.password = (*new_value).clone(),
                                SelectedField::Field(field) => {
                                    entry.fields[*field].1 = (*new_value).clone()
                                }
                                SelectedField::New => unreachable!(),
                            }
                        }
                        _ => return true,
                    }
                    // now we have to save the data
                    let (mut key, mut iv) = EMPTY_KEY;
                    if !get_key(&mut key, &mut iv, data.password_hash) {
                        return true;
                    }
                    let mut entry_iv = iv.clone();
                    get_entry_iv(state.selected_entry.unwrap_entry(), &mut entry_iv);
                    let mut encrypted = Vec::new();
                    let mut encrypter =
                        EncryptWriter::new(&mut encrypted, *entry_iv, key.as_slice(), FLUSH_SOURCE)
                            .unwrap();
                    entry.to_binary(&mut encrypter).unwrap();
                    std::mem::drop(encrypter);
                    data.entries[state.selected_entry.unwrap_entry()] = encrypted;
                    data.save(&key, &iv);
                    data.decrypt_entry(state.selected_entry.unwrap_entry(), &key, &entry_iv, entry);
                } else {
                    print!("\x1b[H");
                    match selected_field {
                        SelectedField::Password => print!("{}", entry.password.as_str()),
                        SelectedField::Field(field) => {
                            print!("{}", entry.fields[*field].1.as_str())
                        }
                        SelectedField::New => unreachable!(),
                    }
                    std::io::stdout().flush().unwrap();
                    std::thread::sleep(std::time::Duration::from_secs(10));
                }
            } else {
                if let SelectedEntry::Entry(entry) = state.selected_entry
                    && state.search.as_ref().map(|s| s.as_str()) == Some("/edit")
                {
                    state.search = None;
                    let new_name = input("New name");
                    let (mut key, mut iv) = EMPTY_KEY;
                    if new_name.is_empty() || !get_key(&mut key, &mut iv, data.password_hash) {
                        return true;
                    }
                    data.names[entry] = new_name;
                    data.save(&key, &iv);
                    return true;
                }
                match state.selected_entry {
                    SelectedEntry::Entry(index) => {
                        let (mut key, mut iv) = (DropShred::new([0; 240]), DropShred::new([0; 16]));
                        if !get_key(&mut key, &mut iv, data.password_hash) {
                            return false;
                        }
                        get_entry_iv(index, &mut iv);
                        state.selected_field = Some((
                            SelectedField::Password,
                            DropShred::new(Entry {
                                password: String::new(),
                                fields: Vec::new(),
                                clipboard_rule: ClipboardRule::Deny,
                            }),
                        ));

                        data.decrypt_entry(
                            index,
                            &key,
                            &iv,
                            &mut state.selected_field.as_mut().unwrap().1,
                        );
                    }
                    SelectedEntry::New => {
                        // We make a new entry!

                        // First we get the name
                        let name = input("Entry name");
                        if name.is_empty() {
                            return true;
                        }

                        // Then we get the password
                        let mut password = DropShred::new(String::new());
                        print!(
                            "\x1b[H\x1b[0K{}Password to store: \x1b[0m",
                            Style::new().background_yellow()
                        );
                        std::io::stdout().flush().unwrap();
                        if !input_hidden(&mut password) {
                            return true;
                        }
                        if password.is_empty() {
                            return true;
                        }
                        let new_entry = DropShred::new(Entry {
                            password: (*password).clone(),
                            fields: Vec::new(),
                            clipboard_rule: ClipboardRule::Deny,
                        });

                        // Now we encrypt it
                        let (mut key, mut iv) = (DropShred::new([0; 240]), DropShred::new([0; 16]));
                        if !get_key(&mut key, &mut iv, data.password_hash) {
                            return true;
                        }
                        let mut entry_iv = iv.clone();
                        get_entry_iv(data.names.len(), &mut entry_iv);

                        let mut encrypted = Vec::new();
                        let mut encrypter = EncryptWriter::new(
                            &mut encrypted,
                            *entry_iv,
                            key.as_slice(),
                            FLUSH_SOURCE,
                        )
                        .unwrap();

                        new_entry.to_binary(&mut encrypter).unwrap();
                        std::mem::drop(encrypter);

                        data.names.push(name);
                        data.entries.push(encrypted);

                        data.save(&key, &iv);
                    }
                }
            }
            true
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SelectedEntry {
    Entry(usize),
    New,
}
impl SelectedEntry {
    fn increment(&mut self, num_entries: usize) {
        *self = match self {
            Self::Entry(entry) => {
                if (*entry + 1) < num_entries {
                    Self::Entry(*entry + 1)
                } else {
                    Self::New
                }
            }
            Self::New => {
                if num_entries > 0 {
                    Self::Entry(0)
                } else {
                    Self::New
                }
            }
        }
    }
    fn decriment(&mut self, num_entries: usize) {
        *self = match self {
            Self::Entry(entry) => {
                if *entry > 0 {
                    Self::Entry(*entry - 1)
                } else {
                    Self::New
                }
            }
            Self::New => {
                if num_entries > 0 {
                    Self::Entry(num_entries - 1)
                } else {
                    Self::New
                }
            }
        }
    }
    fn unwrap_entry(self) -> usize {
        if let Self::Entry(entry) = self {
            entry
        } else {
            panic!("Someone did a fucky")
        }
    }
}
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum SelectedField {
    Password,
    Field(usize),
    New,
}
impl SelectedField {
    fn increment(&mut self, num_fields: usize) {
        *self = match self {
            Self::Password => {
                if num_fields > 0 {
                    Self::Field(0)
                } else {
                    Self::New
                }
            }
            Self::Field(field) => {
                if (*field + 1) < num_fields {
                    Self::Field(*field + 1)
                } else {
                    Self::New
                }
            }
            Self::New => Self::Password,
        }
    }
    fn decriment(&mut self, num_fields: usize) {
        *self = match self {
            Self::Password => Self::New,
            Self::Field(field) => {
                if *field > 0 {
                    Self::Field(*field - 1)
                } else {
                    Self::Password
                }
            }
            Self::New => {
                if num_fields > 0 {
                    Self::Field(num_fields - 1)
                } else {
                    Self::Password
                }
            }
        }
    }
    fn unwrap_field(self) -> usize {
        if let SelectedField::Field(field) = self {
            return field;
        } else {
            panic!("Nuh uh")
        }
    }
}
fn get_terminal_size() -> (usize, usize) {
    (
        String::from_utf8(
            std::process::Command::new("tput")
                .arg("cols")
                .stderr(std::process::Stdio::inherit())
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .parse()
        .expect("This NEEDS stderr to be the terminal in order to work"),
        String::from_utf8(
            std::process::Command::new("tput")
                .arg("lines")
                .stderr(std::process::Stdio::inherit())
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .parse()
        .expect("This NEEDS stderr to be the terminal in order to work"),
    )
}
// Returns if it was successful
#[must_use]
fn get_key(
    key: &mut DropShred<[u8; 240]>,
    iv: &mut DropShred<[u8; 16]>,
    password_hash: [u8; 64],
) -> bool {
    print!(
        "\x1b[H\x1b[0K{}Password:\x1b[0m ",
        Style::new().background_red().intense_background(true)
    );
    std::io::stdout().flush().unwrap();
    let mut password = DropShred::new(String::new());
    if !input_hidden(&mut password) {
        return false;
    }
    if sha512(password.as_bytes()) != password_hash {
        // PUNISHMENT
        let _ = std::process::Command::new("pmset").arg("sleepnow").output();
        return false;
    }
    password_to_key(&password, key, iv);
    true
}
// Retuns if it was successful
#[must_use]
fn input_hidden(output: &mut DropShred<String>) -> bool {
    weirdify();
    // Just shredding to begin with since it needs to be empty
    output.shred_contents();
    let mut stdin = std::io::stdin();
    let mut buf = [0];
    loop {
        stdin.read_exact(&mut buf).unwrap();
        // Start of control sequence
        if buf[0] == 0x1B {
            // Purge stdin
            while let Ok(_) = stdin.read(&mut buf) {}
            return false;
        }
        // Delete
        else if buf[0] == 0x7F {
            if output.is_empty() {
                continue;
            }
            output.pop().unwrap();
            print!("\x1b[D \x1b[D");
            std::io::stdout().flush().unwrap();
        }
        // Enter
        else if buf[0] == b'\n' {
            return true;
        }
        // Appending to the held data
        else if buf[0].is_ascii_graphic() {
            output.push(buf[0] as char);
            print!("*");
            std::io::stdout().flush().unwrap();
        }
    }
}
#[must_use]
fn input_hidden_prompt(prompt: &str, output: &mut DropShred<String>) -> bool {
    print!(
        "\x1b[H\x1b[0K{}{prompt}\x1b[0m: ",
        Style::new().background_yellow()
    );
    std::io::stdout().flush().unwrap();
    input_hidden(output)
}
fn input(prompt: &str) -> String {
    print!(
        "\x1b[H\x1b[0K{}{prompt}\x1b[0m: ",
        Style::new().background_yellow()
    );
    std::io::stdout().flush().unwrap();
    normalize();
    abes_nice_things::input()
}
