use super::*;

fn text(field: &ComboField, at: usize) -> Result<()> {
    if field.text(at).is_none_or(|s| s.trim().is_empty()) {
        bail!("{:?} argument {at} needs a nonempty string", field.keyword);
    }
    Ok(())
}

fn number(field: &ComboField, at: usize) -> Result<()> {
    if field.number(at).is_none() {
        bail!("{:?} argument {at} needs a number", field.keyword);
    }
    Ok(())
}

fn strings(field: &ComboField, first: usize) -> Result<()> {
    for at in first..field.args.len() {
        text(field, at)?;
    }
    Ok(())
}

fn scalar(field: &ComboField) -> Result<()> {
    if field.args.is_empty() {
        return Ok(());
    }
    // Retail TimeStart("-12", "Frames") remains raw text, not -12.0.
    let raw_time_string = field.keyword == K::TimeStart
        && field
            .text(0)
            .is_some_and(|s| s.parse::<f32>().is_ok_and(f32::is_finite));
    if !raw_time_string {
        number(field, 0)?;
    }
    strings(field, 1)
}

pub(super) fn arguments(field: &ComboField) -> Result<()> {
    for arg in &field.args {
        if matches!(arg, ComboValue::Number(n) if !n.is_finite()) {
            bail!("{:?} contains a nonfinite number", field.keyword);
        }
    }
    match field.keyword {
        K::Animation
        | K::State
        | K::ThrowAnimation
        | K::RemapReloadButton
        | K::AttackId
        | K::AimType => {
            text(field, 0)?;
            strings(field, 1)?;
        }
        K::OffhandAnimation => {
            text(field, 0)?;
            text(field, 1)?;
            // The DLC Ventress throw record contains eleven finite numeric
            // trailing slots after its clip/channel strings. Preserve those
            // raw slots; the animator consumes only the first two arguments.
        }
        K::SyncType | K::Button => {
            text(field, 0)?;
            text(field, 1)?;
            strings(field, 2)?;
        }
        K::Sound => {
            text(field, 0)?;
            if field.args.len() > 1 {
                number(field, 1)?;
            }
            strings(field, 2)?;
        }
        K::BlendTimeTo => {
            text(field, 0)?;
            number(field, 1)?;
            strings(field, 2)?;
        }
        K::DeflectAnimation => {
            text(field, 0)?;
            strings(field, 1)?;
        }
        K::Transition => {
            text(field, 0)?;
            for at in 1..field.args.len() {
                number(field, at)?;
            }
        }
        K::Energy | K::Thrust | K::TimeInPosture | K::VelocityXZ => {
            text(field, 0)?;
            number(field, 1)?;
            strings(field, 2)?;
        }
        K::VelocityY | K::VelocityYAbs if field.text(0).is_some() => {
            text(field, 0)?;
            number(field, 1)?;
            strings(field, 2)?;
        }
        K::DamageTime | K::DeflectAngle | K::ThrustAngle | K::VelocityXZAngle => {
            number(field, 0)?;
            number(field, 1)?;
            strings(field, 2)?;
        }
        K::InputLock => {
            let first = usize::from(field.number(0).is_some());
            strings(field, first)?;
        }
        K::Posture | K::ButtonsPressed | K::ButtonsReleased => strings(field, 0)?,
        K::AnimatedMove | K::TurnOffLightsaber => {
            for at in 0..field.args.len() {
                number(field, at)?;
            }
        }
        K::AlignedToThrust | K::Loop => {
            if field.number(0).is_some() {
                strings(field, 1)?;
            } else {
                strings(field, 0)?;
            }
        }
        K::Attack
        | K::Deflect
        | K::If
        | K::Or
        | K::Until
        | K::RestartAnimation
        | K::MustShowOneFrame
        | K::PlayExplosion => {
            if !field.args.is_empty() {
                bail!("{:?} takes no arguments", field.keyword);
            }
        }
        K::Duration
        | K::Break
        | K::TimeStart
        | K::TimeEnd
        | K::BlendInTime
        | K::BlendOutTime
        | K::BlendTimeIn
        | K::BlendTimeOut
        | K::LowResPose
        | K::TurnFactor
        | K::StrafeFactor
        | K::ThrustFactor
        | K::EnergyRestoreRate
        | K::VelocityX
        | K::VelocityY
        | K::VelocityYAbs
        | K::VelocityZ
        | K::VelocityFromThrust
        | K::VelocityFromStrafe
        | K::Gravity
        | K::GravityVelocityTarget
        | K::Damage
        | K::DamageLength
        | K::DamageWidth
        | K::DamageDistance
        | K::Push
        | K::Edge
        | K::EnergyCost => scalar(field)?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dlc_offhand_trailing_slots_do_not_hide_required_clip_and_channel() {
        let mut field = ComboField {
            keyword: K::OffhandAnimation,
            args: vec![
                ComboValue::Text("stand_throw".into()),
                ComboValue::Text("FIRE2".into()),
                ComboValue::Number(-0.0066241594),
            ],
            scoped: false,
            children: vec![],
        };
        assert!(arguments(&field).is_ok());
        field.args[0] = ComboValue::Number(1.0);
        assert!(arguments(&field).is_err());
        field.args[0] = ComboValue::Text("stand_throw".into());
        field.args[1] = ComboValue::Text(String::new());
        assert!(arguments(&field).is_err());
    }
}
