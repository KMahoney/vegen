use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

pub struct Point<T> {
    inner: Rc<RefCell<Node<T>>>,
}

enum Node<T> {
    Info(Info<T>),
    Link(Point<T>),
}

struct Info<T> {
    id: usize,
    rank: u32,
    descriptor: T,
}

impl<T> Clone for Point<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<T> Point<T> {
    pub(crate) fn new(id: usize, descriptor: T) -> Self {
        Self {
            inner: Rc::new(RefCell::new(Node::Info(Info {
                id,
                rank: 0,
                descriptor,
            }))),
        }
    }

    pub fn id(&self) -> usize {
        let node = self.inner.borrow();
        match &*node {
            Node::Info(info) => info.id,
            Node::Link(parent) => parent.id(),
        }
    }
}

impl<T> PartialEq for Point<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T> Eq for Point<T> {}

impl<T> fmt::Debug for Point<T>
where
    T: fmt::Debug + Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if redundant(self) {
            write!(f, "Point(link -> {:?})", find_root(self))
        } else {
            let desc = get(self);
            write!(f, "Point#{}({:?})", self.id(), desc)
        }
    }
}

impl<T> fmt::Display for Point<T>
where
    T: fmt::Display + Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let desc = get(self);
        write!(f, "[{}]", desc)
    }
}

fn find_root<T>(point: &Point<T>) -> Point<T> {
    let parent = {
        let node = point.inner.borrow();
        match &*node {
            Node::Info(_) => return point.clone(),
            Node::Link(parent) => parent.clone(),
        }
    };

    let root = find_root(&parent);
    {
        let mut node = point.inner.borrow_mut();
        *node = Node::Link(root.clone());
    }
    root
}

pub fn fresh<T>(id: usize, descriptor: T) -> Point<T> {
    Point::new(id, descriptor)
}

pub fn get<T>(point: &Point<T>) -> T
where
    T: Clone,
{
    let root = find_root(point);
    let descriptor = {
        let node = root.inner.borrow();
        match &*node {
            Node::Info(info) => info.descriptor.clone(),
            Node::Link(_) => unreachable!("root cannot be a link"),
        }
    };
    descriptor
}

pub fn set<T>(point: &Point<T>, descriptor: T) {
    let root = find_root(point);
    let mut node = root.inner.borrow_mut();
    match &mut *node {
        Node::Info(info) => info.descriptor = descriptor,
        Node::Link(_) => unreachable!("root cannot be a link"),
    }
}

pub fn union<T>(a: &Point<T>, b: &Point<T>, descriptor: T)
where
    T: Clone,
{
    let root_a = find_root(a);
    let root_b = find_root(b);

    if root_a == root_b {
        set(&root_a, descriptor);
        return;
    }

    let rank_a = {
        let node = root_a.inner.borrow();
        match &*node {
            Node::Info(info) => info.rank,
            Node::Link(_) => unreachable!("root cannot be a link"),
        }
    };
    let rank_b = {
        let node = root_b.inner.borrow();
        match &*node {
            Node::Info(info) => info.rank,
            Node::Link(_) => unreachable!("root cannot be a link"),
        }
    };

    let (parent, child, bump_rank) = match rank_a.cmp(&rank_b) {
        std::cmp::Ordering::Greater => (root_a.clone(), root_b.clone(), false),
        std::cmp::Ordering::Less => (root_b.clone(), root_a.clone(), false),
        std::cmp::Ordering::Equal => (root_a.clone(), root_b.clone(), true),
    };

    {
        let mut node = child.inner.borrow_mut();
        *node = Node::Link(parent.clone());
    }

    if bump_rank {
        let mut node = parent.inner.borrow_mut();
        match &mut *node {
            Node::Info(info) => info.rank += 1,
            Node::Link(_) => unreachable!("parent cannot be a link"),
        }
    }

    set(&parent, descriptor);
}

pub fn redundant<T>(point: &Point<T>) -> bool {
    matches!(&*point.inner.borrow(), Node::Link(_))
}
