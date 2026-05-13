use std::path::PathBuf;

#[path = "../src/events.rs"]
mod events;

#[test]
fn loads_real_yaml() {
    let yaml = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("questionnaire/abfi-1/events.yaml");
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("questionnaire/abfi-1/data");
    let course = events::Course::load(&yaml, &data).expect("yaml must parse");
    assert!(!course.events.is_empty(), "course must have events");

    let start = course.first().expect("has first event");
    assert_eq!(start.id, "start");
    assert!(
        !start.buttons.is_empty(),
        "start event must have a button to begin"
    );

    let day1 = course.index_of("day_1").expect("day_1 present");
    assert!(day1 > 0);

    let practice1 = course.index_of("practice_1").expect("practice_1 present");
    let after_day1 = course
        .next_after_button(day1, "message_1")
        .expect("fallback should resolve next-in-order even for typo");
    assert_eq!(
        after_day1, practice1,
        "typo 'message_1' must fall back to next-in-order = practice_1"
    );
}
