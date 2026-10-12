// r2_diag：E2xxx 所有权/借用诊断逐码触发（AC-R2-2 判据固化）+ R2 可表达子集零诊断
// 码表：specs/r2-ownership.md §8；负例构造与定码：shared/ZOR-r2-E2xxx码表对齐-落码前置-v1.md
use zorcc::compile_to_diags;

fn codes(src: &str) -> Vec<String> {
    compile_to_diags(src).iter().map(|d| d.code.clone()).collect()
}
fn first_code(src: &str) -> String {
    compile_to_diags(src).first().map(|d| d.code.clone()).unwrap_or_default()
}
fn clean(src: &str) {
    let d = compile_to_diags(src);
    assert!(d.is_empty(), "expected clean, got: {:?}", d.iter().map(|x| x.render("<t>")).collect::<Vec<_>>());
}

// ———— R2 正例：§7 可表达清单零诊断 ————

#[test]
fn pos_box_create_read_unbox() {
    clean("fn main() { let b = box 1; let v = *b; println(v); unbox b; }");
}

#[test]
fn pos_move_chain_to_arg_and_return() {
    clean("fn take(v: Box<i64>) -> i64 { let x = *v; unbox v; x }\nfn main() { let b = box 5; let r = take(b); println(r); }");
}

#[test]
fn pos_borrow_read_write_params() {
    // 实参位匿名借用=瞬时；绑定句柄才持久（R2 §2"句柄自创建处…块尾"）
    clean("fn read(x: &i64) -> i64 { *x }\nfn bump(x: &mut i64) { *x = 1 }\nfn main() { let a: i64 = 5; let v = read(&a); println(v); bump(&mut a); println(a); }");
}

#[test]
fn pos_borrow_block_end_deactivation() {
    // 块尾失活：内层块共享借用出块即失效，外层可再独占
    clean("fn main() { let x: i64 = 1; { let r = &x; println(*r); } let m = &mut x; *m = 2; println(*m); }");
}

#[test]
fn pos_partial_move_then_restore_whole() {
    // §1：字段移出后整体不可用；再赋整值恢复
    clean("struct Inner { v: Box<i64> }\nstruct Outer { item: Inner, tag: i64 }\nfn main() { let o = Outer { item: Inner { v: box 1 }, tag: 2 }; let x = o.item; println(o.tag); println(x); let o2 = Outer { item: Inner { v: box 3 }, tag: 4 }; println(o2.tag); }");
}

#[test]
fn pos_box_recursive_list_breaks_cycle() {
    // §7 可表达：Box 间址递归类型（E1401 依赖环被 Box 打断，类型层零诊断）
    clean("struct Node { val: i64, next: Box<Node> }\nfn main() { }");
}

#[test]
fn pos_assign_move_between_bindings() {
    clean("fn main() { let a = box 1; let b = box 2; b = a; println(*b); }");
}

#[test]
fn pos_field_auto_borrow_read_write() {
    // §5/§44 字段糖：b.field 自动借用（只读=&、写=&mut），受 §2 判定表约束
    clean("struct P { x: i64 }\nfn main() { let b = box P { x: 1 }; println(b.x); b.x = 2; println(b.x); }");
}

#[test]
fn pos_full_lifecycle_construct_borrow_release() {
    // §7 可表达全链：构造→借用→释放 一例（unbox 前置正常生命周期组合）
    clean("struct P { x: i64 }\nfn read(x: &i64) -> i64 { *x }\nfn bump(x: &mut i64) { *x = 9 }\nfn main() { let b = box P { x: 3 }; let v = read(&b.x); println(v); bump(&mut b.x); println(b.x); unbox b; }");
}

// ———— R2 负例：16 条逐码首诊固化（与 tests/specs/negative/r2-n*.zor 同源构造） ————

#[test]
fn neg_r2_n01_move_then_use() {
    assert_eq!(first_code("fn main() { let a = box 1; let b = a; let c = b; println(*c); println(*a); }"), "E2101");
}

#[test]
fn neg_r2_n02_move_into_arg() {
    assert_eq!(first_code("struct Big { v: Box<i64> }\nfn consume(b: Big) { println(*b.v); }\nfn main() { let g = Big { v: box 2 }; consume(g); println(*g.v); }"), "E2101");
}

#[test]
fn neg_r2_n03_partial_move_whole_use() {
    assert_eq!(first_code("struct Inner { v: Box<i64> }\nstruct Outer { item: Inner, tag: i64 }\nfn main() { let o = Outer { item: Inner { v: box 1 }, tag: 2 }; let x = o.item; println(o.tag); println(x); println(o); }"), "E2102");
}

#[test]
fn neg_r2_n04_mut_while_shared_active() {
    assert_eq!(first_code("fn main() { let x: i64 = 10; let r1 = &x; let r2 = &x; let m = &mut x; println(*r1); println(*r2); println(*m); }"), "E2202");
}

#[test]
fn neg_r2_n05_double_mut() {
    assert_eq!(first_code("fn main() { let x: i64 = 3; let a = &mut x; let b = &mut x; println(*a); println(*b); }"), "E2202");
}

#[test]
fn neg_r2_n06_shared_during_mut() {
    assert_eq!(first_code("fn main() { let x: i64 = 3; let m = &mut x; let r = &x; println(*m); println(*r); }"), "E2201");
}

#[test]
fn neg_r2_n07_return_ref_e1502() {
    assert_eq!(first_code("fn leak(x: i64) -> &i64 { &x }\nfn main() { let y: i64 = 1; let r = leak(y); println(*r); }"), "E1502");
    // E2401 别名不实现（对齐 §2.3 择一）
    assert!(!codes("fn leak(x: i64) -> &i64 { &x } fn main() { }").iter().any(|c| c == "E2401"));
}

#[test]
fn neg_r2_n08_ref_in_struct() {
    assert_eq!(first_code("struct Holder { r: &i64 }\nfn main() { let x: i64 = 1; let h = Holder { r: &x }; println(h); }"), "E1501");
}

#[test]
fn neg_r2_n09_self_ref_struct() {
    assert_eq!(first_code("struct Link { next: &Link }\nfn main() { let a: i64 = 1; let l = Link { next: &a }; println(l); }"), "E1501");
}

#[test]
fn neg_r2_n10_branch_move_conservative() {
    assert_eq!(first_code("fn take(v: Box<i64>) { }\nfn main() { let b = box 1; let flag = true; if flag { take(b); } println(*b); }"), "E2101");
}

#[test]
fn neg_r2_n11_use_after_unbox() {
    assert_eq!(first_code("fn main() { let b = box 1; unbox b; println(*b); }"), "E2301");
}

#[test]
fn neg_r2_n12_deref_moved_box() {
    assert_eq!(first_code("fn main() { let b = box 1; let c = b; println(*b); println(*c); }"), "E2101");
}

#[test]
fn neg_r2_n13_into_slot_then_use() {
    assert_eq!(first_code("struct Slot { owned: Box<i64> }\nfn main() { let b = box 7; let s = Slot { owned: b }; println(*b); println(s.owned); }"), "E2101");
}

#[test]
fn neg_r2_n14_block_end_stale_mut() {
    // 块尾失活近邻①：共享借用最后使用后仍活跃 → 创建 &mut 拒绝（Rust/NLL 可写）
    assert_eq!(first_code("fn main() { let x: i64 = 1; let r = &x; println(*r); let m = &mut x; println(*m); }"), "E2202");
}

#[test]
fn neg_r2_n15_block_end_stale_shared() {
    // 块尾失活近邻②：&mut 最后使用后仍活跃 → 再共享借用拒绝
    assert_eq!(first_code("fn main() { let x: i64 = 1; let m = &mut x; *m = 20; let r = &x; println(*r); println(x); }"), "E2201");
}

#[test]
fn neg_r2_n16_assign_handoff_then_use() {
    assert_eq!(first_code("fn main() { let b = box 9; let c = box 0; c = b; println(*b); println(*c); }"), "E2101");
}

// ———— 判定表补充边界 ————

#[test]
fn neg_double_free_unbox_twice() {
    assert_eq!(first_code("fn main() { let b = box 1; unbox b; unbox b; }"), "E2301");
}

#[test]
fn neg_write_through_shared_ref() {
    assert!(codes("fn main() { let x: i64 = 1; let r = &x; *r = 2; }").contains(&"E2201".to_string()));
}

#[test]
fn neg_moved_used_in_while_cond() {
    assert_eq!(first_code("fn take(v: Box<i64>) { }\nfn main() { let b = box 1; take(b); while *b > 0 { println(1); } }"), "E2101");
}

#[test]
fn pos_generic_angle_still_e1501() {
    // 仅 Box<..> 特例放行，其余尖括号禁则不变（r1-n11 契约）
    assert!(codes("struct Wrap<T> { v: T } fn main() { }").contains(&"E1501".to_string()));
}
