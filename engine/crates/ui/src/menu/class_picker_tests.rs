use super::*;

#[test]
fn class_picker_navigation_and_back_stay_inside_pause() {
    let model = MenuModel {
        class_picker_available: true,
        cursor: 1,
        ..Default::default()
    };
    let enter = MenuInput {
        confirm: true,
        ..Default::default()
    };
    assert_eq!(navigate(&model, enter).1, vec![MenuAction::OpenClassPicker]);
    let picker = MenuModel {
        class_choices: Some(ClassPickerModel {
            tabs: vec!["BF1".into(), "BF2".into()],
            tab: 0,
            choices: vec![(3, "Trooper".into()), (9, "Hero".into())],
        }),
        cursor: 2,
        ..model
    };
    assert_eq!(navigate(&picker, enter).1, vec![MenuAction::SelectClass(9)]);
    assert_eq!(
        navigate(
            &picker,
            MenuInput {
                left: true,
                ..Default::default()
            }
        )
        .1,
        vec![MenuAction::SetClassTab(1)]
    );
    assert_eq!(
        navigate(
            &picker,
            MenuInput {
                right: true,
                ..Default::default()
            }
        )
        .1,
        vec![MenuAction::SetClassTab(1)]
    );
    let back = MenuInput {
        back: true,
        ..Default::default()
    };
    assert_eq!(navigate(&picker, back).1, vec![MenuAction::Resume]);
    assert_eq!(
        navigate(
            &picker,
            MenuInput {
                down: true,
                ..Default::default()
            }
        )
        .0,
        0
    );
    assert!(!rows(&MenuModel::default()).contains(&Row::ClassPicker));
}
