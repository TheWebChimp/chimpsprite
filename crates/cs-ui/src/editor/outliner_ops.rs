//! Las operaciones del outliner: la fila activa, ocultar, bloquear, abrir y cerrar grupos,
//! renombrar, agrupar, mover filas en el árbol y borrarlas. Cada cambio del modelo es un
//! `SetModel` con el modelo entero (un paso de undo), salvo abrir y cerrar un grupo.

use cs_3d::{Item, SelectMode, Selection};
use cs_core::{Group3D, Model3D, Node, Transform3D};

use super::{CadMode, Editor};

/// Una fila del outliner: un objeto o un grupo (índices en `objects` y `groups`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Row {
    Object(usize),
    Group(usize),
}

/// Dónde cae una fila arrastrada: antes o después de otra (en la misma lista que esa) o
/// adentro de un grupo (al final).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropAt {
    Before(Row),
    After(Row),
    Into(usize),
}

impl Editor {
    /// La fila activa del outliner: el grupo elegido o, si no hay, el objeto elegido.
    pub fn active_row(&self) -> Option<Row> {
        let groups = self.edited_model()?.groups.len();
        match self.model_group.filter(|&g| g < groups) {
            Some(g) => Some(Row::Group(g)),
            None => self.selected_object().map(Row::Object),
        }
    }

    /// Elige la fila activa (`None`: ninguna). En modo Model la selección 3D pasa a modo Object
    /// con los objetos de la fila (un grupo: todos los de adentro) que se ven (lo oculto no se
    /// elige); con `add` (Shift) se suman a la selección o, si ya estaban todos, se quitan (y la
    /// fila queda activa igual).
    pub fn set_active_row(&mut self, row: Option<Row>, add: bool) {
        // Durante un Extrude la selección es la de lo extruido: no cambia.
        if self.extruding() {
            return;
        }
        let Some(model) = self.edited_model() else {
            return;
        };
        let row = row.filter(|&r| row_exists(model, r));
        if row.is_none() && add {
            return;
        }
        let objects: Vec<usize> = row
            .map_or(Vec::new(), |r| row_objects(model, r))
            .into_iter()
            .filter(|&o| !model.object_hidden(o))
            .collect();
        (self.model_object, self.model_group) = match row {
            Some(Row::Object(o)) => (Some(o), None),
            Some(Row::Group(g)) => (None, Some(g)),
            None => (None, None),
        };
        // Como en `select_model_object`: solo si el modo Edit es del modelo que se edita, y en
        // modo Texture la selección se queda en caras.
        if self.model_edit.artboard == Some(self.model_id())
            && self.model_edit.mode == CadMode::Model
        {
            let sel = &self.model_edit.selection;
            let mut items = if add && sel.mode == SelectMode::Object {
                sel.items.clone()
            } else {
                Default::default()
            };
            let all_in = objects.iter().all(|&o| items.contains(&Item::Object(o)));
            if add && !objects.is_empty() && all_in {
                for &o in &objects {
                    items.remove(&Item::Object(o));
                }
            } else {
                items.extend(objects.iter().map(|&o| Item::Object(o)));
            }
            self.model_edit.selection = Selection {
                mode: SelectMode::Object,
                items,
            };
        }
        self.revision += 1;
    }

    /// El ojo de la fila: oculta o muestra el objeto o el grupo. Un paso de undo. Lo que queda
    /// oculto (él o lo de adentro del grupo) sale de la selección 3D.
    pub fn toggle_hidden(&mut self, row: Row) -> bool {
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        let Some(hidden) = (match row {
            Row::Object(o) => model.objects.get_mut(o).map(|ob| &mut ob.hidden),
            Row::Group(g) => model.groups.get_mut(g).map(|gr| &mut gr.hidden),
        }) else {
            return false;
        };
        *hidden = !*hidden;
        let label = if *hidden { "Hide" } else { "Show" };
        if !self.named(label, |e| e.set_model(model)) {
            return false;
        }
        self.drop_hidden_from_selection();
        true
    }

    /// Saca de la selección 3D lo de los objetos ocultos (ellos o un grupo suyo). Va después del
    /// paso que los ocultó: deshacerlo vuelve a la selección de antes.
    fn drop_hidden_from_selection(&mut self) {
        if self.model_edit.artboard != Some(self.model_id()) {
            return;
        }
        let Some(model) = self.edited_model() else {
            return;
        };
        let hidden: Vec<usize> = (0..model.objects.len())
            .filter(|&o| model.object_hidden(o))
            .collect();
        self.model_edit
            .selection
            .items
            .retain(|it| !hidden.contains(&it.object()));
    }

    /// El candado de la fila: bloquea o desbloquea el objeto o el grupo. Un paso de undo. (No se
    /// llama `toggle_locked`: ese es el candado de las capas.)
    pub fn toggle_row_locked(&mut self, row: Row) -> bool {
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        let Some(locked) = (match row {
            Row::Object(o) => model.objects.get_mut(o).map(|ob| &mut ob.locked),
            Row::Group(g) => model.groups.get_mut(g).map(|gr| &mut gr.locked),
        }) else {
            return false;
        };
        *locked = !*locked;
        let label = if *locked { "Lock" } else { "Unlock" };
        self.named(label, |e| e.set_model(model))
    }

    /// Double sided del objeto `o`. Un paso de undo.
    pub fn toggle_double_sided(&mut self, o: usize) -> bool {
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        let Some(obj) = model.objects.get_mut(o) else {
            return false;
        };
        obj.double_sided = !obj.double_sided;
        self.named("Double Sided", |e| e.set_model(model))
    }

    /// Abre o cierra el grupo `g` en el outliner. Es cosa de la vista: cambia el documento
    /// directo, sin paso de undo.
    pub fn toggle_open(&mut self, g: usize) {
        let id = self.model_id();
        let Some(group) = self
            .document
            .artboard_mut(id)
            .and_then(|a| a.model.as_mut())
            .and_then(|m| m.groups.get_mut(g))
        else {
            return;
        };
        group.open = !group.open;
        self.revision += 1;
    }

    /// Renombra la fila (sin los espacios de las puntas). Un nombre vacío o el mismo de antes no
    /// cambian nada. Un paso de undo.
    pub fn rename_row(&mut self, row: Row, name: String) -> bool {
        let name = name.trim();
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        let Some(slot) = (match row {
            Row::Object(o) => model.objects.get_mut(o).map(|ob| &mut ob.name),
            Row::Group(g) => model.groups.get_mut(g).map(|gr| &mut gr.name),
        }) else {
            return false;
        };
        if name.is_empty() || slot == name {
            return false;
        }
        *slot = name.to_owned();
        self.named("Rename", |e| e.set_model(model))
    }

    /// New Group: un grupo nuevo con lo elegido (los objetos elegidos en las vistas y la fila
    /// activa) en el lugar del primero (en el orden del árbol), con todo lo elegido adentro en
    /// ese orden; sin nada elegido, uno vacío al final de la raíz. Queda como fila activa. Un
    /// paso de undo.
    pub fn new_group(&mut self) -> bool {
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        model.normalize_tree();
        let mut chosen: Vec<Row> = self.chosen_objects().into_iter().map(Row::Object).collect();
        chosen.extend(self.active_row());
        // En el orden del árbol; lo que ya va dentro de un grupo elegido se mueve con él.
        let chosen: Vec<Row> = tree_rows(&model.tree)
            .into_iter()
            .filter(|r| chosen.contains(r))
            .filter(|&r| {
                !row_parents(&model, r)
                    .iter()
                    .any(|&g| chosen.contains(&Row::Group(g)))
            })
            .collect();
        let ng = model.groups.len();
        model.groups.push(Group3D {
            name: free_group_name(&model),
            transform: Transform3D::default(),
            hidden: false,
            locked: false,
            open: true,
        });
        let empty = Node::Group {
            group: ng,
            children: Vec::new(),
        };
        match chosen.split_first() {
            None => model.tree.push(empty),
            Some((&first, rest)) => {
                // El grupo nuevo ocupa el lugar del primero; lo demás se saca de donde esté.
                let Some(slot) = find_node(&mut model.tree, first) else {
                    return false;
                };
                let mut children = vec![std::mem::replace(slot, empty)];
                children.extend(rest.iter().filter_map(|&r| take_node(&mut model.tree, r)));
                let Some(Node::Group { children: slot, .. }) =
                    find_node(&mut model.tree, Row::Group(ng))
                else {
                    return false;
                };
                *slot = children;
            }
        }
        if !self.named("New Group", |e| e.set_model(model)) {
            return false;
        }
        self.set_active_row(Some(Row::Group(ng)), false);
        true
    }

    /// Arrastrar una fila: la deja antes o después de otra (en la lista de esa) o al final de un
    /// grupo. Un grupo no entra en sí mismo ni en un descendiente, y una fila no cae junto a sí
    /// misma. Un paso de undo. Lo que queda oculto (dentro de un grupo oculto) sale de la
    /// selección 3D.
    pub fn move_row(&mut self, row: Row, to: DropAt) -> bool {
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        model.normalize_tree();
        let target = match to {
            DropAt::Before(r) | DropAt::After(r) => r,
            DropAt::Into(g) => Row::Group(g),
        };
        if target == row || !row_exists(&model, row) || !row_exists(&model, target) {
            return false;
        }
        // Caer adentro de un grupo `g` (o junto a algo que está adentro) es quedar dentro de `g`:
        // un grupo no puede ir a parar dentro de sí mismo.
        if let Row::Group(g) = row {
            let inside = match to {
                DropAt::Into(t) => model.group_parents(t).contains(&g),
                _ => row_parents(&model, target).contains(&g),
            };
            if inside {
                return false;
            }
        }
        let Some(node) = take_node(&mut model.tree, row) else {
            return false;
        };
        if !insert_node(&mut model.tree, to, node) {
            return false;
        }
        if !self.named("Move", |e| e.set_model(model)) {
            return false;
        }
        self.drop_hidden_from_selection();
        true
    }

    /// Borra la fila: un objeto, o un grupo con todo lo de adentro (objetos y subgrupos). Lo
    /// bloqueado no se borra (ni un grupo con algo bloqueado adentro): `false`, sin paso. Un
    /// paso de undo.
    pub fn delete_row(&mut self, row: Row) -> bool {
        self.delete_rows(&[row])
    }

    /// Supr con el puntero sobre el outliner: borra todas las filas elegidas (los objetos
    /// elegidos en las vistas en modo Object y la fila activa: un grupo, con lo de adentro),
    /// salvo lo bloqueado, en un paso de undo. `false` si no queda nada que borrar.
    pub fn delete_chosen_rows(&mut self) -> bool {
        let mut rows: Vec<Row> = self.chosen_objects().into_iter().map(Row::Object).collect();
        rows.extend(self.active_row());
        self.delete_rows(&rows)
    }

    /// Si el botón Delete del panel hace algo (ver [`Editor::delete_from_panel`]).
    pub fn can_delete_from_panel(&self) -> bool {
        match self.panel_group() {
            Some(g) => self
                .edited_model()
                .is_some_and(|m| !row_protected(m, Row::Group(g))),
            None => self.can_delete_model_selection(),
        }
    }

    /// El botón Delete del panel y Supr en las vistas: con un grupo como fila activa en modo
    /// Object, el grupo entero con lo elegido (como Supr en el outliner; lo bloqueado no); si
    /// no, lo elegido en las vistas.
    pub fn delete_from_panel(&mut self) -> bool {
        match self.panel_group() {
            Some(_) => self.delete_chosen_rows(),
            None => self.delete_model_selection(),
        }
    }

    /// El grupo activo, si el botón Delete (y Supr en las vistas) lo borra: en modo Object y sin
    /// gesto en curso.
    fn panel_group(&self) -> Option<usize> {
        let Some(Row::Group(g)) = self.active_row() else {
            return None;
        };
        let object_mode = self.model_edit.selection.mode == SelectMode::Object;
        (object_mode && self.topology_model().is_some()).then_some(g)
    }

    /// Borra `rows` (los objetos y los grupos con todo lo de adentro), salvo las que no existen o
    /// están protegidas por un candado, en un paso de undo. `false` si no queda ninguna.
    fn delete_rows(&mut self, rows: &[Row]) -> bool {
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        model.normalize_tree();
        let rows: Vec<Row> = rows
            .iter()
            .copied()
            .filter(|&r| row_exists(&model, r) && !row_protected(&model, r))
            .collect();
        if rows.is_empty() {
            return false;
        }
        let mut gone: Vec<usize> = rows.iter().flat_map(|&r| row_objects(&model, r)).collect();
        gone.sort_unstable();
        gone.dedup();
        // Los objetos de mayor a menor (quitar uno corre a los de después).
        for &o in gone.iter().rev() {
            model.remove_object(o);
        }
        // Los grupos, con sus subgrupos (uno adentro de otro que se borra ya salió con él).
        let mut groups = Vec::new();
        for &r in &rows {
            if let Row::Group(g) = r
                && let Some(node) = take_node(&mut model.tree, Row::Group(g))
            {
                node_groups(&node, &mut groups);
            }
        }
        groups.sort_unstable();
        groups.dedup();
        for &gr in groups.iter().rev() {
            remove_group(&mut model, gr);
        }
        // La selección guarda índices de objeto: se acomoda ANTES del paso (que la revalida
        // contra el modelo nuevo), como en `remove_object`.
        let shift = |o: usize| o - gone.iter().filter(|&&r| r < o).count();
        let old = self.model_edit.selection.items.clone();
        self.model_edit.selection.items = old
            .iter()
            .filter(|it| !gone.contains(&it.object()))
            .map(|&it| it.with_object(shift(it.object())))
            .collect();
        // Los conteos de la selección se corren igual (ver `sync_model_edit`).
        let made_on = self.model_edit.made_on.clone();
        self.model_edit.made_on = made_on
            .iter()
            .enumerate()
            .filter(|(i, _)| !gone.contains(i))
            .map(|(_, &c)| c)
            .collect();
        if !self.named("Delete", |e| e.set_model(model)) {
            self.model_edit.selection.items = old;
            self.model_edit.made_on = made_on;
            return false;
        }
        // Deshacer el paso vuelve a lo elegido de antes (no a lo acomodado para él).
        self.step_selection_before(old);
        self.model_group = self.model_group.and_then(|g| {
            (!groups.contains(&g)).then(|| g - groups.iter().filter(|&&r| r < g).count())
        });
        self.model_object = self.model_object.filter(|o| !gone.contains(o)).map(shift);
        self.revision += 1;
        true
    }

    /// La transform de la fila activa y si se puede editar: hay fila activa y la selección 3D
    /// no tiene otros objetos que los suyos (un grupo: todos los de adentro); bloqueada (ella o
    /// un grupo de los que la contienen) se ve pero no se edita (`false`).
    pub fn active_transform(&self) -> Option<(Transform3D, bool)> {
        let model = self.edited_model()?;
        let row = self.active_row()?;
        let transform = match row {
            Row::Object(o) => model.objects.get(o)?.transform,
            Row::Group(g) => model.groups.get(g)?.transform,
        };
        if self.model_edit.artboard == Some(self.model_id()) {
            let mine = row_objects(model, row);
            let sel = &self.model_edit.selection;
            if sel.items.iter().any(|it| !mine.contains(&it.object())) {
                return None;
            }
        }
        Some((transform, !row_locked(model, row)))
    }

    /// Cambia la transform de la fila activa (si se puede: ver [`Editor::active_transform`]).
    /// Los cambios seguidos del mismo `group` (un arrastre) son un solo paso de undo.
    pub fn set_active_transform(&mut self, t: Transform3D, group: eframe::egui::Id) -> bool {
        if !self
            .active_transform()
            .is_some_and(|(_, editable)| editable)
        {
            return false;
        }
        let (Some(row), Some(mut model)) = (self.active_row(), self.edited_model().cloned()) else {
            return false;
        };
        let slot = match row {
            Row::Object(o) => model.objects.get_mut(o).map(|ob| &mut ob.transform),
            Row::Group(g) => model.groups.get_mut(g).map(|gr| &mut gr.transform),
        };
        let Some(slot) = slot else {
            return false;
        };
        *slot = t;
        self.set_model_grouped(model, group)
    }

    /// Si hay algo que duplicar (ver [`Editor::duplicate_active`]).
    pub fn can_duplicate_active(&self) -> bool {
        !self.extruding()
            && (matches!(self.active_row(), Some(Row::Group(_)))
                || self.can_duplicate_model_objects())
    }

    /// Duplicate (el botón del outliner, Ctrl+J y el menú Object): con un grupo como fila activa,
    /// el grupo con lo de adentro; si no, los objetos elegidos.
    pub fn duplicate_active(&mut self) -> bool {
        if !self.can_duplicate_active() {
            return false;
        }
        match self.active_row() {
            Some(Row::Group(g)) => self.duplicate_group(g),
            _ => self.duplicate_model_objects(),
        }
    }

    /// Los objetos elegidos en las vistas (en modo Object, del modelo que se edita).
    fn chosen_objects(&self) -> Vec<usize> {
        let sel = &self.model_edit.selection;
        if sel.mode != SelectMode::Object || self.model_edit.artboard != Some(self.model_id()) {
            return Vec::new();
        }
        let count = self.edited_model().map_or(0, |m| m.objects.len());
        sel.items
            .iter()
            .filter_map(|it| match it {
                Item::Object(o) if *o < count => Some(*o),
                _ => None,
            })
            .collect()
    }
}

fn row_exists(model: &Model3D, row: Row) -> bool {
    match row {
        Row::Object(o) => o < model.objects.len(),
        Row::Group(g) => g < model.groups.len(),
    }
}

/// Si la fila está bloqueada: ella o un grupo de los que la contienen.
fn row_locked(model: &Model3D, row: Row) -> bool {
    match row {
        Row::Object(o) => model.object_locked(o),
        Row::Group(g) => {
            model.groups.get(g).is_some_and(|gr| gr.locked)
                || model
                    .group_parents(g)
                    .iter()
                    .any(|&p| model.groups.get(p).is_some_and(|p| p.locked))
        }
    }
}

/// Si el candado no deja borrar la fila: bloqueada o, un grupo, con algo bloqueado adentro (se
/// iría con él).
fn row_protected(model: &Model3D, row: Row) -> bool {
    row_locked(model, row)
        || match row {
            Row::Object(_) => false,
            Row::Group(g) => {
                model
                    .group_objects(g)
                    .iter()
                    .any(|&o| model.object_locked(o))
                    || model
                        .groups
                        .iter()
                        .enumerate()
                        .any(|(i, gr)| gr.locked && model.group_parents(i).contains(&g))
            }
        }
}

/// Los objetos de la fila (un grupo: todos los de adentro).
fn row_objects(model: &Model3D, row: Row) -> Vec<usize> {
    match row {
        Row::Object(o) => vec![o],
        Row::Group(g) => model.group_objects(g),
    }
}

/// Los grupos que contienen a la fila, del más cercano al de la raíz.
fn row_parents(model: &Model3D, row: Row) -> Vec<usize> {
    match row {
        Row::Object(o) => model.object_parents(o),
        Row::Group(g) => model.group_parents(g),
    }
}

fn is_row(n: &Node, row: Row) -> bool {
    match (n, row) {
        (Node::Object(o), Row::Object(r)) => *o == r,
        (Node::Group { group, .. }, Row::Group(r)) => *group == r,
        _ => false,
    }
}

/// Las filas del árbol en orden (cada grupo antes de lo suyo).
fn tree_rows(nodes: &[Node]) -> Vec<Row> {
    let mut out = Vec::new();
    for n in nodes {
        match n {
            Node::Object(o) => out.push(Row::Object(*o)),
            Node::Group { group, children } => {
                out.push(Row::Group(*group));
                out.extend(tree_rows(children));
            }
        }
    }
    out
}

/// El nodo de la fila en el árbol.
fn find_node(nodes: &mut [Node], row: Row) -> Option<&mut Node> {
    for n in nodes {
        if is_row(n, row) {
            return Some(n);
        }
        if let Node::Group { children, .. } = n
            && let Some(found) = find_node(children, row)
        {
            return Some(found);
        }
    }
    None
}

/// Saca el nodo de la fila del árbol (con lo que tenga adentro).
fn take_node(nodes: &mut Vec<Node>, row: Row) -> Option<Node> {
    if let Some(at) = nodes.iter().position(|n| is_row(n, row)) {
        return Some(nodes.remove(at));
    }
    nodes.iter_mut().find_map(|n| match n {
        Node::Group { children, .. } => take_node(children, row),
        Node::Object(_) => None,
    })
}

/// Mete `node` donde dice `to`. Si no encuentra la fila o el grupo, lo devuelve.
fn insert_at(nodes: &mut Vec<Node>, to: DropAt, node: Node) -> Option<Node> {
    let here = nodes.iter().position(|n| match to {
        DropAt::Before(r) | DropAt::After(r) => is_row(n, r),
        DropAt::Into(_) => false,
    });
    if let Some(at) = here {
        let at = if matches!(to, DropAt::After(_)) {
            at + 1
        } else {
            at
        };
        nodes.insert(at, node);
        return None;
    }
    let mut node = node;
    for n in nodes.iter_mut() {
        if let Node::Group { group, children } = n {
            if to == DropAt::Into(*group) {
                children.push(node);
                return None;
            }
            node = insert_at(children, to, node)?;
        }
    }
    Some(node)
}

/// Mete `node` donde dice `to`. `false` si no encuentra la fila o el grupo.
fn insert_node(nodes: &mut Vec<Node>, to: DropAt, node: Node) -> bool {
    insert_at(nodes, to, node).is_none()
}

/// Los grupos de `node` (él, si es uno, y los de adentro).
fn node_groups(node: &Node, out: &mut Vec<usize>) {
    if let Node::Group { group, children } = node {
        out.push(*group);
        for c in children {
            node_groups(c, out);
        }
    }
}

/// Quita el grupo `g` de `groups` (y su nodo del árbol si sigue ahí, con lo de adentro) y corre
/// un lugar los índices de los grupos de después.
fn remove_group(model: &mut Model3D, g: usize) {
    if g >= model.groups.len() {
        return;
    }
    model.groups.remove(g);
    let _ = take_node(&mut model.tree, Row::Group(g));
    fn fix(nodes: &mut [Node], g: usize) {
        for n in nodes {
            if let Node::Group { group, children } = n {
                if *group > g {
                    *group -= 1;
                }
                fix(children, g);
            }
        }
    }
    fix(&mut model.tree, g);
}

/// "Group" o, si ya hay uno con ese nombre, "Group 2", "Group 3"…
fn free_group_name(model: &Model3D) -> String {
    let taken = |name: &str| model.groups.iter().any(|g| g.name == name);
    if !taken("Group") {
        return "Group".to_owned();
    }
    (2..)
        .map(|i| format!("Group {i}"))
        .find(|n| !taken(n))
        .expect("a free name")
}
