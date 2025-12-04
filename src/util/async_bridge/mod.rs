mod command;
pub use command::*;

use crate::prelude::*;

type Channel<T> = (async_channel::Sender<T>, async_channel::Receiver<T>);

#[derive(Resource, Clone)]
pub struct AsyncBridge {
    command_channel: Channel<(CommandQueue, async_channel::Sender<()>)>,
    asset_channel: Channel<(UntypedHandle, async_channel::Sender<Box<dyn Reflect>>)>,
    entity_channel: Channel<(u32, async_channel::Sender<SmallVec<[Entity; 1]>>)>,
}

impl AsyncBridge {
    pub fn ctx(&self) -> AsyncContext {
        AsyncContext {
            command: AsyncMessager(self.command_channel.0.clone()),
            asset: AsyncMessager(self.asset_channel.0.clone()),
            entity: AsyncMessager(self.entity_channel.0.clone()),
        }
    }
}

impl Default for AsyncBridge {
    fn default() -> Self {
        Self {
            command_channel: async_channel::unbounded(),
            asset_channel: async_channel::unbounded(),
            entity_channel: async_channel::unbounded(),
        }
    }
}

#[derive(Clone)]
pub struct AsyncContext {
    pub command: AsyncMessager<CommandQueue, ()>,
    pub asset: AsyncMessager<UntypedHandle, Box<dyn Reflect>>,
    pub entity: AsyncMessager<u32, SmallVec<[Entity; 1]>>,
}

pub struct AsyncMessager<In, Out>(async_channel::Sender<(In, async_channel::Sender<Out>)>);
impl<In, Out> AsyncMessager<In, Out> {
    pub async fn send(&self, input: In) -> Result<Out> {
        let (tx, rx) = async_channel::bounded(1);
        self.0.send((input, tx)).await.map_err(|_| "Channel closed.")?;
        Ok(rx.recv().await?)
    }
}

impl<In> AsyncMessager<In, Box<dyn Reflect>> {
    pub async fn send_typed<T: Any>(&self, input: In) -> Result<T> {
        Ok(*self.send(input).await?.downcast().map_err(|_| "Wrong type.")?)
    }
}

impl<Out> AsyncMessager<(), Out> {
    pub async fn recv(&self) -> Result<Out> {
        self.send(()).await
    }
}

impl<In, Out> Clone for AsyncMessager<In, Out> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

fn execute_async_commands(mut commands: Commands, bridge: Res<AsyncBridge>) {
    let mut count = 128u32;
    while let Some(next_count) = count.checked_sub(1)
        && let Ok((mut queue, done)) = bridge.command_channel.1.try_recv()
    {
        count = next_count;

        queue.push(move |_: &mut World| _ = done.try_send(()));
        commands.append(&mut queue);
    }
}

fn execute_async_assets(mut commands: Commands, bridge: Res<AsyncBridge>, registry: Res<AppTypeRegistry>) -> Result {
    let registry = registry.clone();
    let mut count = 128u32;
    while let Some(next_count) = count.checked_sub(1)
        && let Ok((handle, sender)) = bridge.asset_channel.1.try_recv()
    {
        count = next_count;
        let asset_fns = registry
            .read()
            .get_type_data::<ReflectAsset>(handle.type_id())
            .ok_or("Missing `ReflectAsset`.")?
            .clone();

        commands.queue(move |world: &mut World| -> Result {
            let asset = asset_fns.remove(world, handle.id()).ok_or("Missing asset.")?;
            _ = sender.try_send(asset);

            Ok(())
        });
    }
    Ok(())
}

fn execute_async_entities(entities: &Entities, bridge: Res<AsyncBridge>) {
    let mut count = 128u32;
    while let Some(next_count) = count.checked_sub(1)
        && let Ok((len, sender)) = bridge.entity_channel.1.try_recv()
    {
        count = next_count;
        let entities = entities.reserve_entities(len).collect();

        _ = sender.try_send(entities);
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<AsyncBridge>()
        .add_systems(PreUpdate, (execute_async_commands, execute_async_assets, execute_async_entities));
}
