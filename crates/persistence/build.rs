fn main() {
    // Recompilar inclusive quando uma migração é adicionada, não só quando um .rs muda.
    println!("cargo:rerun-if-changed=migrations");
}
