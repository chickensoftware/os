mod gdt;

pub(super) fn setup() {
    gdt::initialize();
}