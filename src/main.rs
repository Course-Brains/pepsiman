// File format (at file named passwords):
//  password sha512: u64
//  num entries: usize
//  names: [String] encrypted after reversing with base IV
//  entries: [Entry] each entry encrypted individually with their own IV
// The way you get the next IV to use is with the given sha128 function
// entries will be encrypted with len (flush) data
//
// Store public rsa key to output 256 bytes in file named rsa

use abes_nice_things::aes::{DecryptReader, EncryptWriter, FlushSource};
use abes_nice_things::input;
use abes_nice_things::shred::{DropShred, Shred};
use abes_nice_things::{FromBinary, ToBinary};
use std::fs::File;
use std::io::{Read, Write};
mod tui;

const FILE_NAME: &str = "passwords/passwords";
const TEMP_FILE: &str = "passwords/temp_passwords";
const RSA_FILE: &str = "passwords/rsa";
const FLUSH_SOURCE: FlushSource = FlushSource::URandom;

fn main() {
    let normalizer = abes_nice_things::OnDrop::new(normalize);
    ensure_file_availability();
    let mut password = DropShred::new(String::new());
    println!("Please input your password");
    input_hidden(&mut password);
    let (mut key, mut iv) = (DropShred::new([0; 240]), DropShred::new([0; 16]));
    password_to_key(&password, &mut key, &mut iv);
    let password_hash = sha512(password.as_bytes());
    std::mem::drop(password);
    let mut data = Data::load(&key, &iv, password_hash);
    //terminal_menu(&mut data);
    tui::tui(&mut data);
    std::mem::drop(normalizer);
}
// Typing menu at top line for searching and passwords and stuff
//  It will have a prompt when needed like "field name:" and when getting normal data it will have
//  the prompt be highlighted green and then typing password red
//  The terminal cursor will be at the top line at all times
// left column for choosing entry and showing which entry is chosen
// middle column for choosing password or fields and showing which is chosen
// right column for displaying data
// when choosing entry or field, the last thing will be a different color and will create a new one
// typing when choosing will fuzzy search and reorder the list
// All must be able to handle multi line things properly
fn terminal_menu(data: &mut Data) {
    const HELP: &str = "help - This menu\n\
        list - list all entries\n\
        select [id] - select the given entry\n\
        new - create a new entry\n\
        quit - Do I need to explain this to you?";
    println!("{HELP}");
    loop {
        normalize();
        let input = input();
        let mut input = input.split(' ');
        match input.next().unwrap() {
            "" | "help" | "h" => println!("{HELP}"),
            "list" | "l" => {
                for (id, name) in data.names.iter().enumerate() {
                    println!("{id}: {name}")
                }
            }
            "select" | "s" => {
                if let Some(Ok(id)) = input.next().map(|s| s.parse())
                    && id < data.names.len()
                {
                    entry_menu(data, id);
                    println!("{HELP}");
                }
            }
            "new" | "n" => {
                normalize();
                println!("What to name the entry?");
                let name = abes_nice_things::input();
                assert!(!name.is_empty());
                println!("What password to store?");
                let mut entry_password = DropShred::new(String::new());
                input_hidden(&mut entry_password);
                assert!(!entry_password.is_empty());
                data.names.push(name);
                let entry = DropShred::new(Entry {
                    password: (*entry_password).clone(),
                    fields: Vec::new(),
                    clipboard_rule: ClipboardRule::Deny,
                });
                let mut password = DropShred::new(String::new());
                input_password(&mut password, data.password_hash);
                let (mut key, mut iv) = (DropShred::new([0; 240]), DropShred::new([0; 16]));
                password_to_key(&password, &mut key, &mut iv);
                let last_index = data.entries.len();
                let mut entry_iv = iv.clone();
                get_entry_iv(last_index, &mut entry_iv);
                data.entries.push(Vec::new());
                let mut encrypter = EncryptWriter::new(
                    &mut data.entries[last_index],
                    *entry_iv,
                    key.as_slice(),
                    FLUSH_SOURCE,
                )
                .unwrap();
                entry.to_binary(&mut encrypter).unwrap();
                encrypter.flush().unwrap();
                std::mem::drop(encrypter);
                println!("Saving");
                data.save(&key, &iv);
                *data = Data::load(&key, &iv, data.password_hash);
                println!("Done");
            }
            "quit" | "q" => break,
            _ => {}
        }
    }
}
fn entry_menu(data: &mut Data, id: usize) {
    let mut password = DropShred::new(String::new());
    input_password(&mut password, data.password_hash);
    println!("Correct");
    let mut entry = DropShred::new(Entry {
        password: String::new(),
        fields: Vec::new(),
        clipboard_rule: ClipboardRule::Deny,
    });
    //data.decrypt_entry(id, &password, &mut entry);
    const HELP: &str = "help - This menu\n\
        show password - Show the password for 10 seconds\n\
        list - List all field names\n\
        show field [id] - Show the given field for 10 seconds\n\
        new - Create a new field\n\
        back - Go back to the main menu";
    fn show_password(entry: &DropShred<Entry>) {
        print!("Password: \"{}\"", entry.password.as_str());
        std::io::stdout().flush().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(10));
        println!("\x1b[2K");
    }
    fn show_field(entry: &DropShred<Entry>, field: usize) {
        print!(
            "{}: \"{}\"",
            entry.fields[field].0.as_str(),
            entry.fields[field].1.as_str()
        );
        std::io::stdout().flush().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(10));
        println!("\x1b[2K");
    }
    println!("{HELP}");
    loop {
        normalize();
        let input = input();
        let mut input = input.split(" ");
        match input.next().unwrap() {
            "" | "help" | "h" => println!("{HELP}"),
            "show" => {
                let next = input.next();
                if let Some("password") = next {
                    show_password(&entry);
                } else if let Some(Ok(id)) = next.map(|s| s.parse())
                    && id < entry.fields.len()
                {
                    show_field(&entry, id);
                }
            }
            "showpass" | "sp" => show_password(&entry),
            "list" | "l" => {
                for (id, (field, _)) in entry.fields.iter().enumerate() {
                    println!("{id}: {field}");
                }
            }
            "showfield" | "sf" => {
                if let Some(Ok(id)) = input.next().map(|s| s.parse())
                    && id < entry.fields.len()
                {
                    show_field(&entry, id);
                }
            }
            "new" | "n" => {
                println!("What do you want the name of the field to be?");
                normalize();
                let name = abes_nice_things::input();
                if name.is_empty() {
                    println!("Stop wasting my time");
                    continue;
                }
                println!("What do you want it to store?");
                let mut value = DropShred::new(String::new());
                input_hidden(&mut value);
                if value.is_empty() {
                    println!("Stop wasting my time");
                    continue;
                }
                println!("Writing");
                entry.fields.push((name, (*value).clone()));
                let (mut key, mut iv) = (DropShred::new([0; 240]), DropShred::new([0; 16]));
                password_to_key(&password, &mut key, &mut iv);
                let mut entry_iv = iv.clone();
                get_entry_iv(id, &mut entry_iv);
                data.entries[id] = Vec::new();
                let mut encrypter = EncryptWriter::new(
                    &mut data.entries[id],
                    *entry_iv,
                    key.as_slice(),
                    FLUSH_SOURCE,
                )
                .unwrap();
                entry.to_binary(&mut encrypter).unwrap();
                std::mem::drop(encrypter);
                //data.decrypt_entry(id, &password, &mut entry);
                println!("Saving");
                data.save(&key, &iv);
                println!("Done")
            }
            "back" | "cancel" | "b" | "c" | "leave" | "exit" => return,
            _ => {}
        }
    }
}

fn ensure_file_availability() {
    if !std::fs::exists(FILE_NAME).unwrap() {
        // This is the first run or the file got deleted, in which case treat it as a first run
        first_run()
    } else if !std::fs::exists(RSA_FILE).unwrap() {
        // The rsa file is gone but we have the encrypted file. It is now lost data
        panic!("Due to missing rsa file, your passwords cannot be decrypted. Good luck.")
    }
}
fn first_run() {
    normalize();
    // First we create the password directory
    std::fs::create_dir("passwords").unwrap();
    // Second we create the rsa file
    assert!(
        std::process::Command::new("openssl")
            .args(["genrsa", "-out", "private", "2048"])
            .spawn()
            .unwrap()
            .wait()
            .unwrap()
            .success()
    );
    assert!(
        std::process::Command::new("openssl")
            .args(["rsa", "-in", "private", "-pubout", "-out", RSA_FILE])
            .spawn()
            .unwrap()
            .wait()
            .unwrap()
            .success()
    );
    // Remember to shred before deleting
    let private_len = std::fs::metadata("private").unwrap().len();
    std::fs::write("private", vec![0; private_len as usize]).unwrap();
    std::fs::remove_file("private").unwrap();

    // We have to have them pick the password
    println!("What do you want to be your password?");
    let mut password = DropShred::new(String::new());
    input_hidden(&mut password);
    assert!(!password.is_empty());
    println!("Please input it again");
    let mut check = DropShred::new(String::new());
    input_hidden(&mut check);
    assert_eq!(password, check, "The inputted passwords did not match");

    // Now we have the password fully set up
    let password_hash = sha512(password.as_bytes());

    // Now we write the empty data to the file
    let mut file = std::fs::File::create(TEMP_FILE).unwrap();
    file.write_all(&password_hash).unwrap();
    0_usize.to_binary(&mut file).unwrap();
    std::fs::rename(TEMP_FILE, FILE_NAME).unwrap();
    // Now everything is set up
}

struct Data {
    password_hash: [u8; 64],
    names: DropShred<Vec<String>>,
    entries: Vec<Vec<u8>>,
}
impl Data {
    fn save(&self, key: &DropShred<[u8; 240]>, iv: &DropShred<[u8; 16]>) {
        // We create a temp file so that the actual file is never invalid
        let mut file = File::create(TEMP_FILE).unwrap();
        // The password hash is stored decrypted
        self.password_hash.to_binary(&mut file).unwrap();
        // the number of entries is also stored decrypted
        assert_eq!(self.names.len(), self.entries.len());
        self.names.len().to_binary(&mut file).unwrap();

        let mut encrypter =
            EncryptWriter::new(&mut file, **iv, key.as_slice(), FLUSH_SOURCE).unwrap();
        // We already have the number of names stored
        for name in self.names.iter() {
            name.to_binary(&mut encrypter).unwrap();
        }

        // we need the lengths of the entries, which will be encrypted
        for entry in self.entries.iter() {
            entry.len().to_binary(&mut encrypter).unwrap();
        }
        encrypter.flush().unwrap();
        std::mem::drop(encrypter);

        // Entries are already encrypted so they just need to be written
        for entry in self.entries.iter() {
            assert!(entry.len().is_multiple_of(16));
            file.write_all(entry.as_slice()).unwrap();
        }
        std::fs::rename(TEMP_FILE, FILE_NAME).unwrap();
        // Now we push to github or wherever the remote is idk
        if std::fs::exists("passwords/.git").is_ok_and(|a| a) {
            assert!(
                std::process::Command::new("git")
                    .args(["-C", "passwords", "add", "."])
                    .status()
                    .unwrap()
                    .success()
            );
            assert!(
                std::process::Command::new("git")
                    .args([
                        "-C",
                        "passwords",
                        "commit",
                        "--allow-empty-message",
                        "--no-edit"
                    ])
                    .status()
                    .unwrap()
                    .success()
            );
            assert!(
                std::process::Command::new("git")
                    .args(["-C", "passwords", "push"])
                    .status()
                    .unwrap()
                    .success()
            )
        }
    }
    fn load(key: &DropShred<[u8; 240]>, iv: &DropShred<[u8; 16]>, password_hash: [u8; 64]) -> Data {
        let mut file = File::open(FILE_NAME).unwrap();

        let mut stored_hash = [0; 64];
        file.read_exact(&mut stored_hash).unwrap();
        if stored_hash != password_hash {
            // If the hashes do not match then sleep and exit the program
            sleep();
            panic!();
        }

        let num_entries = usize::from_binary(&mut file).unwrap();
        let mut decrypter = DecryptReader::new(&mut file, **iv, key.as_slice()).unwrap();
        let mut names = DropShred::new(Vec::with_capacity(num_entries));
        for _ in 0..num_entries {
            names.push(String::from_binary(&mut decrypter).unwrap());
        }
        let mut entry_lens = Vec::with_capacity(num_entries);
        for _ in 0..num_entries {
            entry_lens.push(usize::from_binary(&mut decrypter).unwrap());
        }
        decrypter.dump_buffer();
        std::mem::drop(decrypter);

        // now we get the entries
        let mut entries = Vec::with_capacity(num_entries);
        for len in entry_lens.into_iter() {
            let mut buf = vec![0; len];
            file.read_exact(&mut buf).unwrap();
            entries.push(buf);
        }

        Data {
            password_hash,
            names,
            entries,
        }
    }
    fn decrypt_entry(
        &self,
        entry: usize,
        key: &DropShred<[u8; 240]>,
        entry_iv: &DropShred<[u8; 16]>,
        output: &mut DropShred<Entry>,
    ) {
        // We don't need to shred this because it contains only encrypted data
        let mut decrypter =
            DecryptReader::new(self.entries[entry].as_slice(), **entry_iv, key.as_slice()).unwrap();
        *output = DropShred::new(Entry::from_binary(&mut decrypter).unwrap());
    }
}

struct Entry {
    password: String,
    fields: Vec<(String, String)>,
    clipboard_rule: ClipboardRule,
}
impl Shred for Entry {
    unsafe fn shred(&mut self) {
        unsafe {
            self.password.shred();
            self.fields.shred();
            self.clipboard_rule.shred();
        }
    }
}
impl ToBinary for Entry {
    fn to_binary(&self, binary: &mut dyn Write) -> std::io::Result<()> {
        self.password.to_binary(binary)?;
        self.fields.to_binary(binary)?;
        self.clipboard_rule.to_binary(binary)
    }
}
impl FromBinary for Entry {
    fn from_binary(binary: &mut dyn Read) -> std::io::Result<Self> {
        Ok(Entry {
            password: String::from_binary(binary)?,
            fields: <Vec<(String, String)>>::from_binary(binary)?,
            clipboard_rule: ClipboardRule::from_binary(binary)?,
        })
    }
}
enum ClipboardRule {
    Allow,
    Deny,
    Timer(std::time::Duration),
}
impl Shred for ClipboardRule {
    unsafe fn shred(&mut self) {
        unsafe { abes_nice_things::shred::shred(self) }
    }
}
impl ToBinary for ClipboardRule {
    fn to_binary(&self, binary: &mut dyn Write) -> std::io::Result<()> {
        match self {
            Self::Allow => 0_u8.to_binary(binary),
            Self::Deny => 1_u8.to_binary(binary),
            Self::Timer(timer) => {
                2_u8.to_binary(binary)?;
                timer.to_binary(binary)
            }
        }
    }
}
impl FromBinary for ClipboardRule {
    fn from_binary(binary: &mut dyn Read) -> std::io::Result<Self> {
        Ok(match u8::from_binary(binary)? {
            0 => Self::Allow,
            1 => Self::Deny,
            2 => Self::Timer(std::time::Duration::from_binary(binary)?),
            other => {
                return Err(std::io::Error::other(format!(
                    "Got invalid discriminant {other:02x} when attempting to get ClipboardRule"
                )));
            }
        })
    }
}
/// This is a specialized function for running commands which take in all of stdin before
/// outputting anything and you will always want a specific amount of output bytes from. AKA the
/// hashing commands, only use this with the hashing commands.
fn run_with_stdin<const N: usize>(command: &str, input: &[u8]) -> [u8; N] {
    let mut command = std::process::Command::new(command);
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::null());
    let mut child = command.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    stdin.write_all(input).unwrap();
    stdin.flush().unwrap();
    std::mem::drop(stdin);

    let mut output = [0; N];
    stdout.read_exact(&mut output).unwrap();
    assert!(child.wait().unwrap().success());
    output
}
/// Under the hood this is sha256 but getting every other byte
fn sha128(input: &[u8]) -> [u8; 16] {
    let output = run_with_stdin::<64>("sha256", input);
    let output = str::from_utf8(&output).unwrap();

    let mut out = [0; 16];
    for i in 0..16 {
        out[i] = u8::from_str_radix(&output[(i * 4)..=(i * 4 + 1)], 16).unwrap();
    }
    out
}
fn sha512(input: &[u8]) -> [u8; 64] {
    let output: [u8; 128] = run_with_stdin("sha512", input);
    let output = str::from_utf8(&output).unwrap();

    let mut out = [0; 64];
    for i in 0..64 {
        out[i] = u8::from_str_radix(&output[(i * 2)..=(i * 2 + 1)], 16).unwrap();
    }
    out
}
fn password_to_key(
    password: &DropShred<String>,
    key: &mut DropShred<[u8; 240]>,
    iv: &mut DropShred<[u8; 16]>,
) {
    let mut command = std::process::Command::new("openssl");
    command.args(["rsautl", "-encrypt", "-pubin", "-inkey", RSA_FILE, "-raw"]);
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::null());
    let mut child = command.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for i in 0..256 {
        if password.as_bytes().len() > i {
            stdin.write_all(&password.as_bytes()[i..=i]).unwrap();
        } else {
            stdin.write_all(&[255 - i as u8]).unwrap();
        }
    }
    stdin.flush().unwrap();
    std::mem::drop(stdin);
    let mut output = DropShred::new([0; 256]);
    child
        .stdout
        .take()
        .unwrap()
        .read_exact(output.as_mut_slice())
        .unwrap();
    assert!(child.wait().unwrap().success());
    for i in 0..240 {
        key[i] = output[i];
    }
    for i in 0..16 {
        iv[i] = output[i + 240]
    }
}
fn get_entry_iv(entry: usize, iv: &mut DropShred<[u8; 16]>) {
    for _ in 0..(entry + 1) {
        **iv = sha128(iv.as_slice());
    }
}
fn sleep() {
    std::process::Command::new("pmset")
        .arg("sleepnow")
        .output()
        .unwrap();
}
fn input_hidden(output: &mut DropShred<String>) {
    weirdify();
    let mut buf = DropShred::new([0]);
    let mut stdin = std::io::stdin();
    stdin.read_exact(buf.as_mut_slice()).unwrap();
    while buf[0] != b'\n' {
        let ch = buf[0] as char;
        // Delete
        if buf[0] == 0x7F && output.len() > 0 {
            output.pop().unwrap();
            print!("\x1b[D \x1b[D");
            std::io::stdout().flush().unwrap();
        } else if buf[0] == 0x1B {
            panic!("Seriously? Come on, you know better");
        } else if ch.is_ascii_graphic() || buf[0] == b' ' {
            output.push(ch);
            print!("*");
            std::io::stdout().flush().unwrap();
        }
        stdin.read_exact(buf.as_mut_slice()).unwrap();
    }
    println!();
}
fn input_password(password: &mut DropShred<String>, hash: [u8; 64]) {
    println!("Please input password");
    input_hidden(password);
    if sha512(password.as_bytes()) != hash {
        sleep();
        panic!("Incorrect");
    }
}

static IS_WEIRD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn weirdify() {
    if IS_WEIRD.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    assert!(
        std::process::Command::new("stty")
            .arg("-echo")
            .arg("-icanon")
            .status()
            .unwrap()
            .success()
    )
}
fn normalize() {
    if !IS_WEIRD.swap(false, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    assert!(
        std::process::Command::new("stty")
            .arg("echo")
            .arg("icanon")
            .status()
            .unwrap()
            .success()
    )
}
