//! Ядро мода: типы сундуков, хранилище содержимого, рецепты апгрейда.
//!
//! Аналог мода IronChest для Minecraft Horizon (Mod Studio).
//! Ванильные сундуки Horizon не поддерживают расширение инвентаря на уровне API,
//! поэтому мод хранит содержимое каждого сундука в собственном персистентном
//! хранилище (`data/contents.dat`), а выдачу предметов игроку выполняет
//! через `spawn_item_to_player` — ровно так же, как это делает оригинальный
//! IronChest (при разбивании сундука выпадают все его предметы).

use std::collections::HashMap;

/// Типы сундуков. Число — количество слотов (аналог IronChest / CraftStorages).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChestType {
    Vanilla,     // 27
    Iron,        // 54
    Gold,        // 81 (9 рядов)
    Diamond,     // 108
    Obsidian,    // 180 (как в IronChest)
    Crystal,     // 216 (аналог кристального сундука CraftStorages)
}

impl ChestType {
    pub const ALL: [ChestType; 6] = [
        ChestType::Vanilla,
        ChestType::Iron,
        ChestType::Gold,
        ChestType::Diamond,
        ChestType::Obsidian,
        ChestType::Crystal,
    ];

    /// Количество слотов хранения.
    pub fn slots(self) -> usize {
        match self {
            ChestType::Vanilla => 27,
            ChestType::Iron => 54,
            ChestType::Gold => 81,
            ChestType::Diamond => 108,
            ChestType::Obsidian => 180,
            ChestType::Crystal => 216,
        }
    }

    /// Имя предмета-сундука в терминах Horizon (префикс `mod:` — namespace мода).
    pub fn item_name(self) -> &'static str {
        match self {
            ChestType::Vanilla => "chest",
            ChestType::Iron => "mod:iron_chest",
            ChestType::Gold => "mod:gold_chest",
            ChestType::Diamond => "mod:diamond_chest",
            ChestType::Obsidian => "mod:obsidian_chest",
            ChestType::Crystal => "mod:crystal_chest",
        }
    }

    pub fn display_name_ru(self) -> &'static str {
        match self {
            ChestType::Vanilla => "Сундук",
            ChestType::Iron => "Железный сундук",
            ChestType::Gold => "Золотой сундук",
            ChestType::Diamond => "Алмазный сундук",
            ChestType::Obsidian => "Обсидиановый сундук",
            ChestType::Crystal => "Кристальный сундук",
        }
    }

    /// Следующий уровень апгрейда (железо → золото → алмаз → обсидиан → кристалл).
    pub fn next(self) -> Option<ChestType> {
        use ChestType::*;
        Some(match self {
            Vanilla => Iron,
            Iron => Gold,
            Gold => Diamond,
            Diamond => Obsidian,
            Obsidian => Crystal,
            Crystal => return None,
        })
    }

    pub fn from_item_name(name: &str) -> Option<ChestType> {
        ChestType::ALL.iter().copied().find(|t| t.item_name() == name)
    }
}

/// Одинаковый предмет в стеке слота.
#[derive(Debug, Clone)]
pub struct ItemStack {
    pub name: String,
    pub count: i32,
    pub data: i32,
}

impl ItemStack {
    pub fn new(name: impl Into<String>, count: i32, data: i32) -> Self {
        Self { name: name.into(), count, data }
    }
}

/// Ключ позиции блока в мире.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPos {
    pub dim: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub fn new(dim: i32, x: i32, y: i32, z: i32) -> Self {
        Self { dim, x, y, z }
    }
    fn key(self) -> String {
        format!("{}/{}/{}/{}", self.dim, self.x, self.y, self.z)
    }
}

/// Запись: какой сундук стоит в позиции и что в нём лежит.
#[derive(Debug, Clone)]
pub struct ChestRecord {
    pub chest_type: ChestType,
    /// Слоты содержимого (не плотнее, чем `chest_type.slots()`).
    pub items: Vec<Option<ItemStack>>,
}

impl ChestRecord {
    pub fn empty(chest_type: ChestType) -> Self {
        Self { chest_type, items: vec![None; chest_type.slots()] }
    }

    /// Кладёт стек: сначала докладывает в существующие стопки такого же
    /// предмета, затем занимает пустые слоты. Возвращает false, если всё
    /// не поместилось (изменения при этом остаются — как в реальном сундуке).
    pub fn insert(&mut self, stack: &ItemStack, max_stack: i32) -> bool {
        let slots = self.chest_type.slots();
        self.items.resize(slots, None);
        let mut left = stack.count;
        // Сначала докладываем в существующие стопки такого же предмета.
        for slot in self.items[..slots].iter_mut() {
            if left <= 0 {
                break;
            }
            if let Some(s) = slot {
                if s.name == stack.name && s.data == stack.data && s.count < max_stack {
                    let take = (max_stack - s.count).min(left);
                    s.count += take;
                    left -= take;
                }
            }
        }
        // Потом — в пустые слоты.
        for slot in self.items[..slots].iter_mut() {
            if left <= 0 {
                break;
            }
            if slot.is_none() {
                let put = left.min(max_stack);
                *slot = Some(ItemStack { count: put, ..stack.clone() });
                left -= put;
            }
        }
        left <= 0
    }

    pub fn total_items(&self) -> usize {
        self.items.iter().filter(|s| s.is_some()).count()
    }
}

/// Персистентное хранилище всех сундуков мода.
#[derive(Default)]
pub struct ChestStorage {
    chests: HashMap<String, ChestRecord>,
}

impl ChestStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn place(&mut self, pos: BlockPos, chest_type: ChestType) {
        self.chests.insert(pos.key(), ChestRecord::empty(chest_type));
    }

    pub fn get(&self, pos: BlockPos) -> Option<&ChestRecord> {
        self.chests.get(&pos.key())
    }

    pub fn get_mut(&mut self, pos: BlockPos) -> Option<&mut ChestRecord> {
        self.chests.get_mut(&pos.key())
    }

    pub fn remove(&mut self, pos: BlockPos) -> Option<ChestRecord> {
        self.chests.remove(&pos.key())
    }

    /// Апгрейд сундука: проверяет рецепт, расширяет хранилище, возвращает
    /// тип нового сундука. `upgrade_item` — предмет, использованный при
    /// постановке (например `mod:ingot_gold`).
    pub fn upgrade_at(&mut self, pos: BlockPos, used_item: &str) -> Option<ChestType> {
        let cur = self.chests.get(&pos.key())?.chest_type;
        let next = cur.next()?;
        // Проверяем, что использованный предмет — «ключ» следующего уровня.
        if upgrade_material_of(next) != Some(used_item) {
            return None;
        }
        let rec = self.chests.get_mut(&pos.key())?;
        rec.chest_type = next;
        rec.items.resize(next.slots(), None);
        Some(next)
    }

    // ---------- Сериализация в собственный формат (.dat) ----------

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"IRCH1"); // магический заголовок + версия
        let n = self.chests.len() as u32;
        out.extend_from_slice(&n.to_le_bytes());
        for (key, rec) in &self.chests {
            push_str(&mut out, key);
            out.push(ChestType::ALL.iter().position(|t| *t == rec.chest_type).unwrap() as u8);
            let cnt = rec.items.len() as u16;
            out.extend_from_slice(&cnt.to_le_bytes());
            for slot in &rec.items {
                match slot {
                    None => out.push(0),
                    Some(s) => {
                        out.push(1);
                        push_str(&mut out, &s.name);
                        out.extend_from_slice(&s.count.to_le_bytes());
                        out.extend_from_slice(&s.data.to_le_bytes());
                    }
                }
            }
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut p = Parser::new(bytes);
        if !p.eat_magic(b"IRCH1") {
            return None;
        }
        let n = p.u32()?;
        let mut chests = HashMap::new();
        for _ in 0..n {
            let key = p.string()?;
            let t = p.u8()? as usize;
            let chest_type = *ChestType::ALL.get(t)?;
            let cnt = p.u16()? as usize;
            let mut items = Vec::with_capacity(cnt);
            for _ in 0..cnt {
                if p.u8()? == 0 {
                    items.push(None);
                } else {
                    items.push(Some(ItemStack { name: p.string()?, count: p.i32()?, data: p.i32()? }));
                }
            }
            chests.insert(key, ChestRecord { chest_type, items });
        }
        Some(Self { chests })
    }
}

/// Предмет-материал, которым «апгрейдится» данный тип сундука.
pub fn upgrade_material_of(t: ChestType) -> Option<&'static str> {
    Some(match t {
        ChestType::Iron => "iron_ingot",
        ChestType::Gold => "gold_ingot",
        ChestType::Diamond => "diamond",
        ChestType::Obsidian => "obsidian",
        ChestType::Crystal => "mod:crystal_shard",
        ChestType::Vanilla => return None,
    })
}

fn push_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u32).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn new(b: &'a [u8]) -> Self { Self { b, i: 0 } }
    fn eat_magic(&mut self, m: &[u8]) -> bool {
        if self.b.len() >= m.len() && &self.b[..m.len()] == m {
            self.i = m.len();
            true
        } else { false }
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let e = self.i.checked_add(n)?;
        if e > self.b.len() { return None; }
        let s = &self.b[self.i..e];
        self.i = e;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> { Some(self.take(1)?[0]) }
    fn u16(&mut self) -> Option<u16> { Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?)) }
    fn u32(&mut self) -> Option<u32> { Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?)) }
    fn i32(&mut self) -> Option<i32> { Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?)) }
    fn string(&mut self) -> Option<String> {
        let l = self.u32()? as usize;
        Some(String::from_utf8(self.take(l)?.to_vec()).ok()?)
    }
}

/// Крафты мода: (результат, массив ингредиентов "name[count]").
/// Регистрируются в рантайме через `add_recipe`.
pub fn recipes() -> Vec<(&'static str, Vec<(&'static str, i32)>)> {
    vec![
        // Железный сундук: сундук + 5 железных слитков
        ("mod:iron_chest", vec![("chest", 1), ("iron_ingot", 5)]),
        // Золотой: железный + 5 золотых слитков
        ("mod:gold_chest", vec![("mod:iron_chest", 1), ("gold_ingot", 5)]),
        // Алмазный: золотой + 5 алмазов
        ("mod:diamond_chest", vec![("mod:gold_chest", 1), ("diamond", 5)]),
        // Обсидиановый: алмазный + 8 обсидиана
        ("mod:obsidian_chest", vec![("mod:diamond_chest", 1), ("obsidian", 8)]),
        // Кристальный: обсидиановый + 4 кристаллических осколка
        ("mod:crystal_chest", vec![("mod:obsidian_chest", 1), ("mod:crystal_shard", 4)]),
        // Осколок из кварца
        ("mod:crystal_shard", vec![("quartz_block", 1)]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_match_ironchest() {
        assert_eq!(ChestType::Vanilla.slots(), 27);
        assert_eq!(ChestType::Iron.slots(), 54);
        assert_eq!(ChestType::Gold.slots(), 81);
        assert_eq!(ChestType::Diamond.slots(), 108);
        assert_eq!(ChestType::Obsidian.slots(), 180);
        assert_eq!(ChestType::Crystal.slots(), 216);
    }

    #[test]
    fn insert_stacks_and_overflow() {
        let mut st = ChestStorage::new();
        let pos = BlockPos::new(0, 10, 64, -5);
        st.place(pos, ChestType::Iron);
        let r = st.get_mut(pos).unwrap();
        assert!(r.insert(&ItemStack::new("dirt", 40, 0), 64));
        assert!(r.insert(&ItemStack::new("dirt", 40, 0), 64)); // сложится в один стек 64 + второй 16
        assert_eq!(r.items[0].as_ref().unwrap().count, 64);
        assert_eq!(r.items[1].as_ref().unwrap().count, 16);
        assert!(!r.insert(&ItemStack::new("stone", 5400, 0), 64)); // переполнение 54 слотов
    }

    #[test]
    fn upgrade_expands_storage() {
        let mut st = ChestStorage::new();
        let pos = BlockPos::new(0, 1, 2, 3);
        st.place(pos, ChestType::Vanilla);
        st.get_mut(pos).unwrap().insert(&ItemStack::new("apple", 10, 0), 64);
        assert_eq!(st.upgrade_at(pos, "iron_ingot"), Some(ChestType::Iron));
        assert_eq!(st.get(pos).unwrap().items.len(), 54);
        assert_eq!(st.upgrade_at(pos, "dirt"), None); // неверный материал
        assert_eq!(st.upgrade_at(pos, "gold_ingot"), Some(ChestType::Gold));
    }

    #[test]
    fn serialization_roundtrip() {
        let mut st = ChestStorage::new();
        let pos = BlockPos::new(1, -64, 100, 2048);
        st.place(pos, ChestType::Diamond);
        st.get_mut(pos).unwrap().insert(&ItemStack::new("mod:crystal_shard", 3, 7), 16);
        let bytes = st.to_bytes();
        let st2 = ChestStorage::from_bytes(&bytes).expect("parse");
        let r = st2.get(pos).unwrap();
        assert_eq!(r.chest_type, ChestType::Diamond);
        assert_eq!(r.items[0].as_ref().unwrap().name, "mod:crystal_shard");
        assert_eq!(r.items[0].as_ref().unwrap().data, 7);
    }
}
