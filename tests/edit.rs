use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};

use tempfile::TempDir;
use yinyang::edit::ini::{get_value, remove_key, remove_section_if_empty, set_value};
use yinyang::edit::jsonc::set_top_level_string;
use yinyang::edit::write_preserving;

// ---- ini ----------------------------------------------------------------------------------

#[test]
fn ini_replaces_only_the_wanted_value() {
    let text = "# comment\n[General]\nUITheme=theme-light\nOther = 1\n\n[Z]\nUITheme=keep\n";
    assert_eq!(
        set_value(text, "General", "UITheme", "theme-dark"),
        "# comment\n[General]\nUITheme=theme-dark\nOther = 1\n\n[Z]\nUITheme=keep\n"
    );
}

#[test]
fn ini_keeps_the_spacing_around_the_equals_sign() {
    assert_eq!(set_value("[A]\nName = Dark\n", "A", "Name", "Light"), "[A]\nName = Light\n");
    assert_eq!(set_value("[A]\nName\t=\tDark\n", "A", "Name", "Light"), "[A]\nName\t=\tLight\n");
}

#[test]
fn ini_keeps_crlf_and_a_missing_final_newline() {
    assert_eq!(set_value("[A]\r\nk=1\r\n", "A", "k", "2"), "[A]\r\nk=2\r\n");
    assert_eq!(set_value("[A]\nk=1", "A", "k", "2"), "[A]\nk=2");
    assert_eq!(set_value("[A]\r\nk=1\r\n", "A", "new", "x"), "[A]\r\nk=1\r\nnew=x\r\n");
}

#[test]
fn ini_replaces_every_copy_of_a_duplicated_key_in_the_section() {
    assert_eq!(set_value("[A]\nk=1\nk=2\n[B]\nk=3\n", "A", "k", "9"), "[A]\nk=9\nk=9\n[B]\nk=3\n");
}

#[test]
fn ini_adds_a_missing_key_at_the_end_of_its_section() {
    assert_eq!(
        set_value("[A]\nx=1\n\n[B]\ny=2\n", "A", "z", "9"),
        "[A]\nx=1\nz=9\n\n[B]\ny=2\n"
    );
    assert_eq!(set_value("[A]\nx=1", "A", "z", "9"), "[A]\nx=1\nz=9\n");
    assert_eq!(set_value("[A]\n", "A", "z", "9"), "[A]\nz=9\n");
}

#[test]
fn ini_adds_a_missing_section_at_the_end_of_the_file() {
    assert_eq!(set_value("[A]\nx=1\n", "B", "k", "v"), "[A]\nx=1\n\n[B]\nk=v\n");
    assert_eq!(set_value("[A]\nx=1", "B", "k", "v"), "[A]\nx=1\n\n[B]\nk=v\n");
    assert_eq!(set_value("", "B", "k", "v"), "[B]\nk=v\n");
}

#[test]
fn ini_get_value_reads_the_effective_uncommented_value() {
    let text = "#k=comment\n;k=comment\n[A]\n#k=no\nk = 1 \nk=2\n[B]\nk=3\n";
    assert_eq!(get_value(text, "A", "k").as_deref(), Some("2"));
    assert_eq!(get_value(text, "B", "k").as_deref(), Some("3"));
    assert_eq!(get_value(text, "A", "missing"), None);
    assert_eq!(get_value(text, "C", "k"), None);
    assert_eq!(get_value("[A]\r\nk= v \r\n", "A", "k").as_deref(), Some("v"));
}

#[test]
fn ini_remove_key_touches_only_the_given_section() {
    assert_eq!(remove_key("[A]\nk=1\nx=2\n[B]\nk=3\n", "A", "k"), "[A]\nx=2\n[B]\nk=3\n");
    assert_eq!(remove_key("[A]\nx=2\n", "A", "k"), "[A]\nx=2\n");
}

#[test]
fn ini_removes_a_section_only_when_it_is_empty() {
    assert_eq!(remove_section_if_empty("[A]\nx=1\n\n[B]\n", "B"), "[A]\nx=1\n");
    assert_eq!(remove_section_if_empty("[A]\nx=1\n\n[B]\n\n[C]\ny=2\n", "B"), "[A]\nx=1\n\n[C]\ny=2\n");
    assert_eq!(remove_section_if_empty("[A]\nx=1\n", "A"), "[A]\nx=1\n", "has a key");
    assert_eq!(remove_section_if_empty("[A]\n# keep me\n", "A"), "[A]\n# keep me\n", "has a comment");
    assert_eq!(remove_section_if_empty("[A]\nx=1\n", "Missing"), "[A]\nx=1\n");
}

#[test]
fn ini_set_then_remove_restores_the_original_file() {
    let original = "[General]\nColor=blue\n";
    let dark = set_value(original, "Document", "ChangeColors", "true");
    assert_eq!(dark, "[General]\nColor=blue\n\n[Document]\nChangeColors=true\n");
    let light = remove_section_if_empty(&remove_key(&dark, "Document", "ChangeColors"), "Document");
    assert_eq!(light, original);
}

// ---- jsonc --------------------------------------------------------------------------------

const KEY: &str = "workbench.colorTheme";

#[test]
fn jsonc_changes_only_the_value_leaving_comments_and_commas() {
    let before = "// my settings\n{\n\t/* editor */\n\t\"editor.fontSize\": 14,\n\t\"workbench.colorTheme\": \"Solarized Light\", // theme\n\t\"files.exclude\": {\"workbench.colorTheme\": \"nested\"},\n}\n";
    let after = before.replacen("Solarized Light", "Solarized Dark", 1);
    assert_eq!(set_top_level_string(before, KEY, "Solarized Dark").unwrap(), after);
}

#[test]
fn jsonc_ignores_the_key_inside_comments_and_strings() {
    let before = "{\n  // \"workbench.colorTheme\": \"in a comment\"\n  /* \"workbench.colorTheme\": \"in a block\" */\n  \"note\": \"\\\"workbench.colorTheme\\\": \\\"in a string\\\"\",\n  \"workbench.colorTheme\": \"real\"\n}\n";
    let after = before.replacen("\"real\"", "\"new\"", 1);
    assert_eq!(set_top_level_string(before, KEY, "new").unwrap(), after);
}

#[test]
fn jsonc_changes_the_first_of_two_top_level_copies() {
    assert_eq!(
        set_top_level_string("{\"workbench.colorTheme\": \"a\", \"workbench.colorTheme\": \"b\"}", KEY, "z").unwrap(),
        "{\"workbench.colorTheme\": \"z\", \"workbench.colorTheme\": \"b\"}"
    );
}

#[test]
fn jsonc_inserts_a_missing_key_in_the_style_of_the_file() {
    assert_eq!(
        set_top_level_string("{\n\t\"a\": 1\n}\n", KEY, "T").unwrap(),
        "{\n\t\"workbench.colorTheme\": \"T\",\n\t\"a\": 1\n}\n"
    );
    assert_eq!(
        set_top_level_string("{\"a\": 1}", KEY, "T").unwrap(),
        "{\"workbench.colorTheme\": \"T\", \"a\": 1}"
    );
}

#[test]
fn jsonc_inserts_a_key_into_an_empty_object_without_a_dangling_comma() {
    assert_eq!(set_top_level_string("{}", KEY, "T").unwrap(), "{\"workbench.colorTheme\": \"T\"}");
    assert_eq!(
        set_top_level_string("{\n}\n", KEY, "T").unwrap(),
        "{\n    \"workbench.colorTheme\": \"T\"\n}\n"
    );
    assert_eq!(
        set_top_level_string("// only a comment first\n{ /* nothing */ }", KEY, "T").unwrap(),
        "// only a comment first\n{ /* nothing */ \"workbench.colorTheme\": \"T\" }"
    );
}

#[test]
fn jsonc_rejects_text_without_a_top_level_object() {
    for bad in ["", "   \n", "[]", "42", "\"str\"", "// only a comment\n", "{ \"a\": 1", "/* open", "{ \"a: 1 }"] {
        assert!(set_top_level_string(bad, KEY, "T").is_err(), "{bad:?} should be rejected");
    }
}

#[test]
fn jsonc_escapes_the_new_value() {
    let out = set_top_level_string("{\"workbench.colorTheme\": \"x\"}", KEY, "a\"b\\c\u{e9}").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed[KEY], "a\"b\\c\u{e9}");
}

#[test]
fn jsonc_replaces_a_scalar_but_refuses_to_replace_a_container() {
    assert_eq!(
        set_top_level_string("{\"workbench.colorTheme\": null}", KEY, "T").unwrap(),
        "{\"workbench.colorTheme\": \"T\"}"
    );
    assert!(set_top_level_string("{\"workbench.colorTheme\": {\"a\": 1}}", KEY, "T").is_err());
    assert!(set_top_level_string("{\"workbench.colorTheme\": [1]}", KEY, "T").is_err());
}

#[test]
fn jsonc_keeps_a_byte_order_mark_and_unicode_elsewhere() {
    let before = "\u{feff}{\"naïve\": \"caffè ☕\", \"workbench.colorTheme\": \"a\"}";
    assert_eq!(
        set_top_level_string(before, KEY, "b").unwrap(),
        "\u{feff}{\"naïve\": \"caffè ☕\", \"workbench.colorTheme\": \"b\"}"
    );
}

// ---- write_preserving ---------------------------------------------------------------------

#[test]
fn write_preserving_keeps_the_file_mode() {
    let dir = TempDir::new().unwrap();
    for mode in [0o644, 0o600] {
        let file = dir.path().join(format!("settings-{mode:o}.json"));
        fs::write(&file, "old").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(mode)).unwrap();
        write_preserving(&file, "new").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "new");
        assert_eq!(fs::metadata(&file).unwrap().permissions().mode() & 0o777, mode);
    }
}

#[test]
fn write_preserving_writes_through_a_symlink_and_keeps_it() {
    let dir = TempDir::new().unwrap();
    let real_dir = dir.path().join("dotfiles");
    fs::create_dir(&real_dir).unwrap();
    let target = real_dir.join("settings.json");
    fs::write(&target, "old").unwrap();
    let link = dir.path().join("settings.json");
    symlink(&target, &link).unwrap();

    write_preserving(&link, "new").unwrap();

    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(&target).unwrap(), "new");
    let leftovers: Vec<_> = fs::read_dir(&real_dir).unwrap().flatten().collect();
    assert_eq!(leftovers.len(), 1, "temporary file left behind");
}

#[test]
fn write_preserving_refuses_a_missing_file() {
    let dir = TempDir::new().unwrap();
    assert!(write_preserving(&dir.path().join("nope.json"), "x").is_err());
    assert!(!dir.path().join("nope.json").exists());
}
