use crate::{
    prelude::*,
    util::{async_bridge::AsyncContext, ecs::RawBundle},
};

impl AsyncContext {
    pub fn commands(&self) -> AsyncCommands<'_> {
        AsyncCommands { ctx: self, queue: default() }
    }
}

pub struct AsyncCommands<'a> {
    ctx: &'a AsyncContext,
    queue: Queue<'a>,
}

impl AsyncCommands<'_> {
    pub fn reborrow(&mut self) -> AsyncCommands<'_> {
        AsyncCommands {
            ctx: self.ctx,
            queue: match self.queue {
                Queue::Owned(ref mut queue) => Queue::Borrowed(queue),
                Queue::Borrowed(ref mut queue) => Queue::Borrowed(queue),
            },
        }
    }

    pub fn queue<C: Command<T> + HandleError<T>, T>(&mut self, command: C) {
        self.queue.push(command.handle_error());
    }

    pub fn queue_handled<C: Command<T> + HandleError<T>, T>(&mut self, command: C, error_handler: fn(BevyError, ErrorContext)) {
        self.queue.push(command.handle_error_with(error_handler));
    }

    pub fn queue_silenced<C: Command<T> + HandleError<T>, T>(&mut self, command: C) {
        self.queue.push(command.ignore_error());
    }

    pub async fn spawn_empty(&mut self) -> Result<AsyncEntityCommands<'_>> {
        let entity = self.ctx.entity.send(1).await?[0];
        Ok(self.entity(entity))
    }

    pub async fn spawn(&mut self, bundle: impl Bundle) -> Result<AsyncEntityCommands<'_>> {
        let entity = self.ctx.entity.send(1).await?[0];
        let mut entity_commands = self.entity(entity);
        entity_commands.insert(bundle);
        Ok(entity_commands)
    }

    pub async fn spawn_many(&self, count: u32) -> Result<SmallVec<[Entity; 1]>> {
        self.ctx.entity.send(count).await
    }

    pub fn entity(&mut self, entity: Entity) -> AsyncEntityCommands<'_> {
        AsyncEntityCommands {
            commands: self.reborrow(),
            entity,
        }
    }

    pub async fn submit(&mut self) -> Result {
        self.ctx.command.send(mem::take(&mut self.queue)).await.map_err(|_| "Channel closed.")?;
        Ok(())
    }
}

pub struct AsyncEntityCommands<'a> {
    commands: AsyncCommands<'a>,
    entity: Entity,
}

impl AsyncEntityCommands<'_> {
    pub fn id(&self) -> Entity {
        self.entity
    }

    pub fn reborrow(&mut self) -> AsyncEntityCommands<'_> {
        AsyncEntityCommands {
            commands: self.commands.reborrow(),
            entity: self.entity,
        }
    }

    pub fn queue<C: EntityCommand<T> + CommandWithEntity<M>, T, M>(&mut self, command: C) -> &mut Self {
        self.commands.queue(command.with_entity(self.entity));
        self
    }

    pub fn queue_handled<C: EntityCommand<T> + CommandWithEntity<M>, T, M>(
        &mut self,
        command: C,
        error_handler: fn(BevyError, ErrorContext),
    ) -> &mut Self {
        self.commands.queue_handled(command.with_entity(self.entity), error_handler);
        self
    }

    pub fn queue_silenced<C: EntityCommand<T> + CommandWithEntity<M>, T, M>(&mut self, command: C) -> &mut Self {
        self.commands.queue_silenced(command.with_entity(self.entity));
        self
    }

    pub fn despawn(mut self) {
        self.queue_handled(entity_command::despawn(), warn);
    }

    pub fn insert(&mut self, bundle: impl Bundle) -> &mut Self {
        self.queue(entity_command::insert(bundle, InsertMode::Replace))
    }

    pub fn insert_raw_bundle(
        &mut self,
        components_iter: impl IntoIterator<Item = Box<dyn Reflect>> + 'static + Send + Sync,
        hook_mode: RelationshipHookMode,
    ) -> &mut Self {
        self.queue(RawBundle::insert(components_iter, hook_mode))
    }
}

impl Drop for AsyncCommands<'_> {
    #[track_caller]
    fn drop(&mut self) {
        if !std::thread::panicking()
            && let Queue::Owned(ref mut queue) = self.queue
            && !queue.is_empty()
        {
            warn!(
                "{}`AsyncCommands` dropped without calling `submit()` first; trying to send the leftovers in a non-blocking way",
                MaybeLocation::caller().map(|loc| format!("{loc}: ")).unwrap_or_default()
            );
            _ = self.ctx.command.try_send(mem::take(&mut self.queue));
        }
    }
}

enum Queue<'a> {
    Owned(CommandQueue),
    Borrowed(&'a mut CommandQueue),
}

impl Deref for Queue<'_> {
    type Target = CommandQueue;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(queue) => queue,
            Self::Borrowed(queue) => queue,
        }
    }
}

impl DerefMut for Queue<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Owned(queue) => queue,
            Self::Borrowed(queue) => queue,
        }
    }
}

impl Default for Queue<'_> {
    fn default() -> Self {
        Self::Owned(default())
    }
}
