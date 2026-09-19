#set heading(numbering: "1.a:")
#set page(numbering: "1")
#show link: underline
#show link: set text(blue)
#show text: set text(size: 14pt)
#title("Storage format for encrypted data in Pepsiman")
Abraham Sharnoff\
Document last updated on #datetime.today().display()
#let aes_link(body) = {
  link("https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.197-upd1.pdf")[#body]
}
#let String = link("https://doc.rust-lang.org/std/string/struct.String.html")[String]
#let Vec = link("https://doc.rust-lang.org/std/vec/struct.Vec.html")[Vec]

For converting information to and from binary to store in
the file, we will be using ToBinary and FromBinary as
defined in
#link("https://github.com/Course-Brains/abes_nice_things/blob/main/src/from_binary.rs")[abes_nice_things].

= Information
First we will look at what information we need to actually
store.
- The #link(<entries>, "entries") themselves
- The names of those entries
- A 64 byte hash of the correct password#footnote[
  Created using sha256]
- A 2048 bit public #link(
  "https://en.wikipedia.org/wiki/RSA_cryptosystem"
  )[RSA] key#footnote[
  Used to get the required data length for the
  #link(<key>)[key] and #link(<iv>)[IV]]
The names of the entries are of course stored as Strings.\
For technical reasons, the RSA key is stored in a separate
file.
== Entries <entries>
Entries contain the password, any number of
#link(<field>)[fields] and
the rule on clipboard allowance, which is currently not
used.\
The password is a #String, obviously.
=== Field <field>
Fields contain a #String for the name of the field as well
as a #String for the value.
=== Clipboard rule
This will be one of the following:
/ Allow: Allow copying without clearing the clipboard.
/ Deny: Deny copying
/ Timer (time): Allow copying but clear the clipboard after the given time has passed

= The unencrypted format
#table(
  [64 byte hash: \[u8; 64\]],
  [Number of entries: usize],
  [Names: \[String\]#footnote[
    This format is for a slice of a given type, which is
    like an array in that it is all next to each other and
    in order but does not store the length]],
  [Entries: \[#link(<entry>)[Entry]\]]
)
Notably the number of names and the number of entries will
always be the same because the names are the names of the
entries.
== Entry <entry>
#table(
  [Password: #String],
  [fields: #Vec\(#String, #String\)],
  [Clipboard rule: #link(<ClipboardRule>)[ClipboardRule]]
)
=== ClipboardRule <ClipboardRule>
$
"enum"cases(
  "Allow: 0_u8",
  "Deny: 1_u8",
  "Timer: [2_u8," #link("https://doc.rust-lang.org/beta/std/time/struct.Duration.html")[Duration]"]"
)
$
= Encryption
The hash is left unencrypted because it needs to be checked
against before decryption.\
The names are encrypted using the calculated key and the
base IV.\
The entries are each encrypted using their own IV which is
determined by their index. Essentially take the base IV and
run it through a modified #link(
  "https://en.wikipedia.org/wiki/SHA-2")[sha256] which takes
every other byte a number of times equal to its index + 1.\
So for the first entry you run it once, for the second you
run it twice and so on.\
Every time a section finishes its own encryption, it does
get padding added if needed sourced through /dev/urandom.
If it ended in such a way that padding is not needed to
create a complete block then it is not padded.

#pagebreak(weak: true)
#title[Glossary]
/ Key: The 240 byte key used for #aes_link[AES-256] <key>
/ IV: The 16 byte initialization vector which is used to block chain #aes_link[AES] <iv>