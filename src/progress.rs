use crate::prelude::*;

#[derive(Resource, Debug)]
pub struct ProgressTracker<T: FreelyMutableState> {
    progresses: Slab<AtomicU64>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: FreelyMutableState> Default for ProgressTracker<T> {
    fn default() -> Self {
        Self {
            progresses: default(),
            _marker: PhantomData,
        }
    }
}

impl<T: FreelyMutableState> ProgressTracker<T> {
    pub fn register(&mut self) -> ProgressId {
        ProgressId(self.progresses.insert(AtomicU64::new(0)))
    }

    pub fn unregister(&mut self, id: ProgressId) {
        self.progresses.try_remove(id.0);
    }

    pub fn contains(&self, id: ProgressId) -> bool {
        self.progresses.contains(id.0)
    }

    pub fn update(&self, id: ProgressId, progress: impl Into<Progress>) {
        self.progresses
            .get(id.0)
            .expect("Progress already unregistered")
            .store(progress.into().to_bits(), Ordering::Relaxed);
    }

    pub fn count_progress(&self) -> Progress {
        let mut progress = Progress::default();
        for (.., p) in &self.progresses {
            let Progress { current, total } = Progress::from_bits(p.load(Ordering::Relaxed));
            progress.current += current;
            progress.total += total;
        }
        progress
    }

    pub fn count_progress_f32(&self) -> Option<f32> {
        let Progress { current, total } = self.count_progress();
        (total != 0).then(|| current as f32 / total as f32)
    }

    pub fn is_finished(&self) -> bool {
        let progress = self.count_progress();
        progress.total > 0 && progress.current >= progress.total
    }

    pub fn reset(&mut self) {
        for (.., p) in &mut self.progresses {
            *p.get_mut() = Progress::INVALID.to_bits();
        }
    }
}

#[derive(Reflect, Debug, Default, Clone, Copy)]
#[reflect(Debug, Default, FromWorld, Clone)]
#[repr(C, align(8))]
pub struct Progress {
    #[cfg(target_endian = "little")]
    pub current: u32,
    /// If this is `0`, the progress becomes invalid and ignored.
    pub total: u32,
    #[cfg(target_endian = "big")]
    pub current: u32,
}

impl Progress {
    pub const INVALID: Self = Progress { current: 0, total: 0 };

    pub fn new(current: u32, total: u32) -> Self {
        Self { current, total }
    }

    #[inline(always)]
    pub const fn from_bits(bits: u64) -> Self {
        unsafe { mem::transmute(bits) }
    }

    #[inline(always)]
    pub const fn to_bits(self) -> u64 {
        unsafe { mem::transmute(self) }
    }
}

impl<Current: TryInto<u32>, Total: TryInto<u32>> From<(Current, Total)> for Progress {
    fn from((current, total): (Current, Total)) -> Self {
        Self {
            current: current.try_into().unwrap_or(0),
            total: total.try_into().unwrap_or(0),
        }
    }
}

impl<T: TryInto<u32>> From<[T; 2]> for Progress {
    fn from([current, total]: [T; 2]) -> Self {
        Self {
            current: current.try_into().unwrap_or(0),
            total: total.try_into().unwrap_or(0),
        }
    }
}

impl From<bool> for Progress {
    fn from(value: bool) -> Self {
        Self {
            current: value.into(),
            total: 1,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProgressId(usize);

#[derive(Debug, Clone, Copy)]
pub struct ProgressFor<'w, T: FreelyMutableState> {
    progress: &'w AtomicU64,
    _marker: PhantomData<fn() -> T>,
}

impl<T: FreelyMutableState> ProgressFor<'_, T> {
    pub fn update(self, progress: impl Into<Progress>) {
        self.progress.store(progress.into().to_bits(), Ordering::Relaxed);
    }
}

unsafe impl<'w, T: FreelyMutableState> SystemParam for ProgressFor<'w, T> {
    type State = (<Res<'w, ProgressTracker<T>> as SystemParam>::State, ProgressId);
    type Item<'world, 'state> = ProgressFor<'world, T>;

    fn init_state(world: &mut World) -> Self::State {
        let tracker_state = Res::<ProgressTracker<T>>::init_state(world);
        (tracker_state, world.resource_mut::<ProgressTracker<T>>().register())
    }

    fn init_access((tracker_state, ..): &Self::State, system_meta: &mut SystemMeta, component_access_set: &mut FilteredAccessSet, world: &mut World) {
        Res::<ProgressTracker<T>>::init_access(tracker_state, system_meta, component_access_set, world);
    }

    unsafe fn get_param<'world, 'state>(
        &mut (tracker_state, id): &'state mut Self::State,
        _: &SystemMeta,
        world: UnsafeWorldCell<'world>,
        _: Tick,
    ) -> Self::Item<'world, 'state> {
        let tracker = unsafe {
            world
                .get_resource_by_id(tracker_state)
                .expect("Resource does not exist")
                .deref::<ProgressTracker<T>>()
        };

        ProgressFor {
            progress: tracker.progresses.get(id.0).expect("Progress already unregistered"),
            _marker: PhantomData,
        }
    }

    unsafe fn validate_param(
        &mut (tracker_state, id): &mut Self::State,
        _: &SystemMeta,
        world: UnsafeWorldCell,
    ) -> Result<(), SystemParamValidationError> {
        let tracker = unsafe {
            world
                .get_resource_by_id(tracker_state)
                .ok_or_else(|| SystemParamValidationError::invalid::<Res<ProgressTracker<T>>>("Resource does not exist"))?
                .deref::<ProgressTracker<T>>()
        };

        if !tracker.contains(id) {
            Err(SystemParamValidationError::invalid::<Self>("Progress already unregistered"))?
        }
        Ok(())
    }

    fn apply((tracker_state, ..): &mut Self::State, system_meta: &SystemMeta, world: &mut World) {
        Res::<ProgressTracker<T>>::apply(tracker_state, system_meta, world);
    }

    fn queue((tracker_state, ..): &mut Self::State, system_meta: &SystemMeta, world: DeferredWorld) {
        Res::<ProgressTracker<T>>::queue(tracker_state, system_meta, world);
    }
}

#[derive(Resource, Debug)]
struct ProgressTransitions<T: FreelyMutableState>(HashMap<T, T>);
impl<T: FreelyMutableState> Default for ProgressTransitions<T> {
    fn default() -> Self {
        Self(default())
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProgressSystems {
    UpdateTransitions,
}

fn update_progress_transitions<T: FreelyMutableState>(
    tracker: Res<ProgressTracker<T>>,
    transitions: Res<ProgressTransitions<T>>,
    from: Res<State<T>>,
    mut next: ResMut<NextState<T>>,
) {
    if let Some(to) = transitions.0.get(&**from)
        && tracker.is_finished()
    {
        next.set(to.clone());
    }
}

fn reset_progresses<T: FreelyMutableState>(mut tracker: ResMut<ProgressTracker<T>>) {
    tracker.reset();
}

pub struct ProgressPlugin<T: FreelyMutableState> {
    schedule: Interned<dyn ScheduleLabel>,
    transitions: HashMap<T, T>,
}

impl<T: FreelyMutableState> ProgressPlugin<T> {
    pub fn new(schedule: impl ScheduleLabel) -> Self {
        Self {
            schedule: schedule.intern(),
            transitions: default(),
        }
    }

    pub fn trans(mut self, from: T, to: T) -> Self {
        self.transitions.insert(from, to);
        self
    }
}

impl<T: FreelyMutableState> Plugin for ProgressPlugin<T> {
    fn build(&self, app: &mut App) {
        app.insert_resource(ProgressTracker::<T>::default())
            .insert_resource(ProgressTransitions(self.transitions.clone()))
            .add_systems(self.schedule, update_progress_transitions::<T>.in_set(ProgressSystems::UpdateTransitions));

        for from in self.transitions.keys() {
            app.add_systems(OnExit(from.clone()), reset_progresses::<T>);
        }
    }
}
