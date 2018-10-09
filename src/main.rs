extern crate string_interner;
mod xml_quote;
mod num_fmt;
use string_interner::{StringInterner, Sym};

use std::io::{self, BufRead, BufReader};
use std::fmt::{Write};
use std::collections::{HashMap};
use xml_quote::{XmlQuote};
use num_fmt::{NumFmt};

#[derive(Debug)]
struct Node {
    count: u64,
    children: Option<HashMap<Sym, Node>>,
}

impl Node {
    pub fn new() -> Node {
        Node {
            count: 0,
            children: None,
        }
    }

    pub fn add<'a, I>(&mut self, path: &mut I, count: u64)
        where I: Iterator<Item=Sym>
    {
        self.count += count;
        if let Some(child_name) = path.next() {
            self.children
                .get_or_insert_with(|| HashMap::new())
                .entry(child_name.into())
                .or_insert_with(|| Node::new())
                .add(path, count);
        }
    }

    pub fn depth(&self, min_count: u64, depth: u64) -> u64 {
        if self.count < min_count {
            return depth;
        }
        if let Some(children) = &self.children {
            children.values().map(|c|c.depth(min_count, depth+1))
                .max().unwrap_or(depth)
        } else {
            depth
        }
    }

    #[allow(dead_code)]
    pub fn print(&self, interner: &StringInterner<Sym>, name: &Sym, depth: usize) {
        println!("{:pad$}{} {}", "", interner.resolve(*name).expect("lost interned string?"), self.count, pad=depth);
        let children = if let Some(c) = &self.children { c } else { return };
        let mut keys: Vec<Sym> = children.keys().cloned().collect();
        keys.sort();
        for k in keys {
            children[&k].print(interner, &k, depth + 1);
        }
    }

    #[allow(dead_code)]
    pub fn gen_rects(&self, name: &Sym, depth: u64, offset: u64, buf: &mut Vec<Rect>) {
        buf.push(Rect {
            name: name.clone(),
            count: self.count,
            depth, offset,
        });
        let children = if let Some(c) = &self.children { c } else { return };
        let mut keys: Vec<Sym> = children.keys().cloned().collect();
        keys.sort();
        let mut delta = 0;
        for k in keys {
            let child = &children[&k];
            child.gen_rects(&k, depth + 1, offset + delta, buf);
            delta += child.count;
        }
    }
}

#[derive(Debug)]
struct Rect {
    name: Sym,
    count: u64,
    depth: u64,
    offset: u64,
}

struct Frame<'a> {
    keys: Vec<Sym>,
    start: u64,
    offset: u64,
    name: Sym,
    node: &'a Node,
}
impl<'a> Frame<'a> {
    pub fn new(node: &'a Node, name: &Sym, offset: u64) -> Frame<'a> {
        let keys = if let Some(children) = &node.children {
            let mut keys: Vec<Sym> = children.keys().cloned().collect();
            keys.sort_by(|a, b| b.cmp(a));
            keys
        } else {
            Vec::new()
        };
        Frame {
            keys, node, offset,
            start: offset,
            name: name.clone(),
        }
    }
}

struct Rects<'a> {
    stack: Vec<Frame<'a>>,
}
impl<'a> Rects<'a> {
    pub fn new(node: &'a Node, name: &Sym) -> Rects<'a> {
        Rects { stack: vec![Frame::new(node, name, 0)] }
    }
}
impl<'a> Iterator for Rects<'a> {
    type Item = Rect;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(mut current) = self.stack.pop() {
            let depth = self.stack.len() as u64;
            if let Some(key) = current.keys.pop() {
                let child = &current.node.children.as_ref().unwrap()[&key];
                let next = Frame::new(child, &key, current.offset);
                current.offset += child.count;
                self.stack.push(current);
                self.stack.push(next);
            } else {
                return Some(Rect {
                    name: current.name,
                    count: current.node.count,
                    offset: current.start,
                    depth: depth,
                });
            }
        }
        None
    }
}

fn main() {
    let stdin = io::stdin();
    let input = BufReader::new(stdin);
    let mut invalid_lines = 0_u64;
    let reverse = false;

    let mut interner = StringInterner::default();
    let mut root = Node::new();
    for line_res in input.lines() {
        let string = if let Ok(line) = line_res {
            line
        } else {
            break;
        };
        let line = string.trim();
        let stack;
        let count_str;
        if let Some(last) = line.rfind(' ') {
            stack = &line[..last];
            count_str = &line[last+1..];
        } else {
            invalid_lines += 1;
            continue;
        };

        let count;
        if let Ok(parsed) = count_str.parse() {
            count = parsed;
        } else {
            invalid_lines += 1;
            continue;
        };

        if reverse {
            root.add(&mut stack.rsplit(';')
                .filter(|s|!s.is_empty())
                .map(|s| interner.get_or_intern(s)), count);
        } else {
            root.add(&mut stack.split(';')
                .filter(|s| !s.is_empty())
                .map(|s| interner.get_or_intern(s)), count);
        };
    }

    let name: Sym = interner.get_or_intern("all");
    if root.count == 0 {
        eprintln!("no valid stack counts provided!");
        return;
    }

    let width = 1910.0_f32;
    let px_per_depth = 20.0_f32;
    let min_width = 0.1_f32;
    let px_per_count = width / (root.count as f32);

    let min_count = (min_width / px_per_count) as u64;
    let max_depth = root.depth(min_count, 0);

    let height = ((max_depth + 1) as f32) * px_per_depth;

    let count_name = "samples";

    let mut output = String::new();

    write!(&mut output, r#"<?xml version="1.0" standalone="no"?>
        <!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
        <svg version="1.1" width="{0}" height="{1}" viewBox="0 0 {0} {1}" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
        <style>rect {{ stroke-width: 0.5; stroke: #ddd; }}
        g:hover rect {{ stroke: black; }}
        text {{ alignment-baseline: before-edge; }}</style>
"#,
        width, height);

    let mut rect_id = 0;
    let upside_down = false;
    for rect in Rects::new(&root, &name) {
        let rect_width = (rect.count as f32) * px_per_count;
        if rect_width < min_width { continue; }
        let y = if upside_down { rect.depth } else { max_depth - rect.depth };
        write!(&mut output,
r#"<g><title>{text} ({count} {count_name} {percent:.1}%)</title>
<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="red" />
<clipPath id="clip{idx}"><rect x="{x}" y="{y}" width="{w}" height="{h}" /></clipPath>
<text x="{x}" y="{y}" clip-path="url(#clip{idx})">{text}</text></g>
"#,
                 x=(rect.offset as f32) * px_per_count,
                 y=(y as f32) * px_per_depth,
                 w=rect_width,
                 h=px_per_depth-1.0,
                 count_name=XmlQuote::new(count_name),
                 count=NumFmt::new(rect.count),
                 percent=100.0*(rect_width / width),
                 text=XmlQuote::new(interner.resolve(rect.name).expect("could not resolve name?")),
                 idx=rect_id);
        rect_id += 1;
    }
    write!(&mut output, r#"</svg>"#);
    println!("{}", output);
    if invalid_lines > 0 {
        eprintln!("invalid lines: {}", invalid_lines);
    }
}
