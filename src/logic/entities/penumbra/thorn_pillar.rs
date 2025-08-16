use crate::{
    PIXELS_PER_UNIT,
    logic::{
        Fields, FromLevelEntity,
        entities::{
            EntityLayers, TryHurt,
            penumbra::{AttractedInitial, PenumbraEntity, TryLaunch},
        },
    },
    prelude::*,
};

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(PenumbraEntity, CollisionLayers = EntityLayers::penumbra_hostile())]
pub struct ThornPillar;
impl FromLevelEntity for ThornPillar {
    type Param = ();
    type Data = Write<Transform>;

    fn from_level_entity(mut e: EntityCommands, fields: &Fields, _: &mut SystemParamItem<Self::Param>, mut trns: QueryItem<Self::Data>) -> Result {
        let length = fields.int("length")?;
        let ccw = fields.bool("ccw")?;
        let facing = fields.point_px("facing")?.as_vec2();

        trns.rotation = Quat::from_axis_angle(Vec3::Z, (facing - trns.translation.xy()).to_angle());
        e.insert((
            Self,
            AttractedInitial { ccw },
            Collider::rectangle(length as f32 * PIXELS_PER_UNIT as f32, PIXELS_PER_UNIT as f32 / 2.),
        ))
        .observe(|mut trigger: Trigger<TryLaunch>, mut commands: Commands| {
            if trigger.by() != trigger.target() {
                trigger.event_mut().stop();
                commands.entity(trigger.by()).queue(TryHurt::by(trigger.target(), 1));
            }
        });

        Ok(())
    }
}
