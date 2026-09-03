//! The Qt adapter for `registration_graph`.
//!
//! This is the ONLY file in the stack that knows Qt exists. It owns a
//! `RegistrationDef` and exposes it to QML as a QAbstractListModel; every
//! actual edit is delegated to the pure domain crate.
//!
//! There is no `revision` counter. Qt's own insert/remove/move notifications
//! do that job, and unlike a manual token they fail loudly (Qt asserts) rather
//! than silently.

use core::pin::Pin;
use cxx_qt::CxxQtType;

use cxx_qt_lib::{QByteArray, QModelIndex, QString, QVariant};
use registration_graph::{CostFunction, RegistrationDef};

/// Role ids. Start at Qt::UserRole (256) so we never collide with
/// Qt::DisplayRole and friends.
const ROLE_NAME: i32 = 256;
const ROLE_COST_FUNCTION: i32 = 257;

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qbytearray.h");
        type QByteArray = cxx_qt_lib::QByteArray;

        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;

        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;

        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;

        include!("cxx-qt-lib/qlist.h");
        type QList_i32 = cxx_qt_lib::QList<i32>;
    }
    unsafe extern "C++Qt" {
        include!(<QtCore/QAbstractListModel>);

        /// Base class for the registration editor's list model.
        #[qobject]
        type QAbstractListModel;
    }

    extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        #[qproperty(i32, selected_stage)]
        type RegistrationEditor = super::RegistrationEditorRust;

        // ---- stage editing -------------------------------------------
        #[qinvokable]
        fn add_stage(self: Pin<&mut RegistrationEditor>);

        #[qinvokable]
        fn remove_stage(self: Pin<&mut RegistrationEditor>, row: i32);

        #[qinvokable]
        fn move_stage_up(self: Pin<&mut RegistrationEditor>, row: i32);

        #[qinvokable]
        fn move_stage_down(self: Pin<&mut RegistrationEditor>, row: i32);

        // ---- cost-function options -----------------------------------
        #[qinvokable]
        fn cost_function_count(self: &RegistrationEditor) -> i32;

        #[qinvokable]
        fn cost_function_name(self: &RegistrationEditor, ordinal: i32) -> QString;

        #[qinvokable]
        fn stage_cost_function_index(self: &RegistrationEditor, row: i32) -> i32;

        #[qinvokable]
        fn set_stage_cost_function(self: Pin<&mut RegistrationEditor>, row: i32, ordinal: i32);
    }

    // -----------------------------------------------------------------
    // QAbstractListModel overrides
    // -----------------------------------------------------------------
    extern "RustQt" {
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &RegistrationEditor, parent: &QModelIndex) -> i32;

        #[qinvokable]
        #[cxx_override]
        fn data(self: &RegistrationEditor, index: &QModelIndex, role: i32) -> QVariant;

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &RegistrationEditor) -> QHash_i32_QByteArray;
    }

    // -----------------------------------------------------------------
    // Base-class methods we need to call. These are the notifications that
    // replace the old revision counter.
    // -----------------------------------------------------------------
    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows(
            self: Pin<&mut RegistrationEditor>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );

        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows(self: Pin<&mut RegistrationEditor>);

        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        unsafe fn begin_remove_rows(
            self: Pin<&mut RegistrationEditor>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );

        #[inherit]
        #[cxx_name = "endRemoveRows"]
        unsafe fn end_remove_rows(self: Pin<&mut RegistrationEditor>);

        #[inherit]
        #[cxx_name = "beginMoveRows"]
        unsafe fn begin_move_rows(
            self: Pin<&mut RegistrationEditor>,
            source_parent: &QModelIndex,
            source_first: i32,
            source_last: i32,
            destination_parent: &QModelIndex,
            destination_child: i32,
        ) -> bool;

        #[inherit]
        #[cxx_name = "endMoveRows"]
        unsafe fn end_move_rows(self: Pin<&mut RegistrationEditor>);

        #[inherit]
        fn index(
            self: &RegistrationEditor,
            row: i32,
            column: i32,
            parent: &QModelIndex,
        ) -> QModelIndex;

        #[inherit]
        #[cxx_name = "dataChanged"]
        unsafe fn data_changed(
            self: Pin<&mut RegistrationEditor>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QList_i32,
        );
    }
}

// -------------------------------------------------------------------------
// Rust state
// -------------------------------------------------------------------------

pub struct RegistrationEditorRust {
    registration: RegistrationDef,
    selected_stage: i32,
}

impl Default for RegistrationEditorRust {
    fn default() -> Self {
        Self {
            registration: RegistrationDef::with_default_stages(3),
            selected_stage: 0,
        }
    }
}

impl ffi::RegistrationEditor {
    // ---------------------------------------------------------------------
    // Handoff to the graph builder (not exposed to QML)
    // ---------------------------------------------------------------------

    pub fn registration(&self) -> &RegistrationDef {
        &self.rust().registration
    }

    // ---------------------------------------------------------------------
    // QAbstractListModel
    // ---------------------------------------------------------------------

    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rust().registration.len() as i32
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(stage) = usize::try_from(index.row())
            .ok()
            .and_then(|row| self.rust().registration.stage(row))
        else {
            return QVariant::default();
        };

        match role {
            ROLE_NAME => QVariant::from(&QString::from(stage.name.as_str())),
            ROLE_COST_FUNCTION => QVariant::from(&QString::from(stage.cost_function.label())),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> ffi::QHash_i32_QByteArray {
        let mut roles = ffi::QHash_i32_QByteArray::default();
        roles.insert(ROLE_NAME, QByteArray::from("name"));
        roles.insert(ROLE_COST_FUNCTION, QByteArray::from("costFunction"));
        roles
    }

    // ---------------------------------------------------------------------
    // Stage editing
    // ---------------------------------------------------------------------

    pub fn add_stage(mut self: Pin<&mut Self>) {
        let row = self.rust().registration.len() as i32;
        let parent = QModelIndex::default();

        unsafe {
            self.as_mut().begin_insert_rows(&parent, row, row);
            self.as_mut().rust_mut().registration.push_default_stage();
            self.as_mut().end_insert_rows();
        }

        self.as_mut().set_selected_stage(row);
    }

    pub fn remove_stage(mut self: Pin<&mut Self>, row: i32) {
        let Some(index) = self.valid_row(row) else {
            return;
        };

        let parent = QModelIndex::default();

        unsafe {
            self.as_mut().begin_remove_rows(&parent, row, row);
            self.as_mut().rust_mut().registration.remove_stage(index);
            self.as_mut().end_remove_rows();
        }

        // Keep the selection on the same *stage*, not the same index.
        let count = self.rust().registration.len() as i32;
        let selected = *self.selected_stage();

        let next_selection = if count == 0 {
            -1
        } else if row < selected {
            (selected - 1).clamp(0, count - 1)
        } else {
            selected.clamp(0, count - 1)
        };

        self.as_mut().set_selected_stage(next_selection);
    }

    pub fn move_stage_up(mut self: Pin<&mut Self>, row: i32) {
        let Some(index) = self.valid_row(row) else {
            return;
        };

        if index == 0 {
            return;
        }

        let parent = QModelIndex::default();

        unsafe {
            // Destination for "up by one" is simply row - 1.
            if !self
                .as_mut()
                .begin_move_rows(&parent, row, row, &parent, row - 1)
            {
                return;
            }

            self.as_mut()
                .rust_mut()
                .registration
                .swap_stages(index, index - 1);

            self.as_mut().end_move_rows();
        }

        self.as_mut().set_selected_stage(row - 1);
    }

    pub fn move_stage_down(mut self: Pin<&mut Self>, row: i32) {
        let Some(index) = self.valid_row(row) else {
            return;
        };

        if index + 1 >= self.rust().registration.len() {
            return;
        }

        let parent = QModelIndex::default();

        unsafe {
            // NOTE the +2. beginMoveRows takes the destination in terms of the
            // list BEFORE the row is lifted out, so moving down by one lands
            // at row + 2, not row + 1. Getting this wrong trips a Qt assert.
            if !self
                .as_mut()
                .begin_move_rows(&parent, row, row, &parent, row + 2)
            {
                return;
            }

            self.as_mut()
                .rust_mut()
                .registration
                .swap_stages(index, index + 1);

            self.as_mut().end_move_rows();
        }

        self.as_mut().set_selected_stage(row + 1);
    }

    // ---------------------------------------------------------------------
    // Cost-function options
    // ---------------------------------------------------------------------

    pub fn cost_function_count(&self) -> i32 {
        CostFunction::ALL.len() as i32
    }

    pub fn cost_function_name(&self, ordinal: i32) -> QString {
        let Some(cost_function) = usize::try_from(ordinal)
            .ok()
            .and_then(CostFunction::from_ordinal)
        else {
            return QString::default();
        };

        QString::from(cost_function.label())
    }

    pub fn stage_cost_function_index(&self, row: i32) -> i32 {
        let Some(stage) = usize::try_from(row)
            .ok()
            .and_then(|row| self.rust().registration.stage(row))
        else {
            return -1;
        };

        stage.cost_function.ordinal() as i32
    }

    pub fn set_stage_cost_function(mut self: Pin<&mut Self>, row: i32, ordinal: i32) {
        let Some(cost_function) = usize::try_from(ordinal)
            .ok()
            .and_then(CostFunction::from_ordinal)
        else {
            return;
        };

        let Some(index) = self.valid_row(row) else {
            return;
        };

        let changed = self
            .as_mut()
            .rust_mut()
            .registration
            .set_cost_function(index, cost_function);

        if !changed {
            return;
        }

        let model_index = self.index(row, 0, &QModelIndex::default());

        unsafe {
            self.as_mut()
                .data_changed(&model_index, &model_index, &ffi::QList_i32::default());
        }
    }

    fn valid_row(&self, row: i32) -> Option<usize> {
        let row = usize::try_from(row).ok()?;
        (row < self.rust().registration.len()).then_some(row)
    }
}
