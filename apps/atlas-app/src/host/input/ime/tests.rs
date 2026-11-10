use super::ImeFlag;

#[test]
fn the_state_is_written_once_and_again_only_when_it_changes() {
    let mut flag = ImeFlag::default();
    assert_eq!(flag.update(false), Some("false"));
    assert_eq!(flag.update(false), None);
    assert_eq!(flag.update(true), Some("true"));
    assert_eq!(flag.update(true), None);
    assert_eq!(flag.update(false), Some("false"));
}
