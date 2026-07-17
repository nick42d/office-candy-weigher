//! Implementation of a select algorithm that uses some external state to poll
//! futures in a round robin order.
//!
//! Note use of pin-project crate: This is used to allow the subfields of these
//! futures to be directly polled (which requires them to be wrapped in pin,
//! which pin-project handles safely for us).
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use embassy_futures::select::{Either, Either3, Either4};
use pin_project::pin_project;

#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[pin_project]
pub struct RoundRobinSelect<'a, A, B> {
    poll_first: &'a mut PollFirst2,
    #[pin]
    a: A,
    #[pin]
    b: B,
}

#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[pin_project]
pub struct RoundRobinSelect3<'a, A, B, C> {
    poll_first: &'a mut PollFirst3,
    #[pin]
    a: A,
    #[pin]
    b: B,
    #[pin]
    c: C,
}

#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[pin_project]
pub struct RoundRobinSelect4<'a, A, B, C, D> {
    poll_first: &'a mut PollFirst4,
    #[pin]
    a: A,
    #[pin]
    b: B,
    #[pin]
    c: C,
    #[pin]
    d: D,
}

#[derive(Copy, Clone, Debug)]
pub enum PollFirst2 {
    A,
    B,
}
impl PollFirst2 {
    pub fn next(&mut self) {
        match self {
            PollFirst2::A => *self = PollFirst2::B,
            PollFirst2::B => *self = PollFirst2::A,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum PollFirst3 {
    A,
    B,
    C,
}
impl PollFirst3 {
    pub fn next(&mut self) {
        match self {
            PollFirst3::A => *self = PollFirst3::B,
            PollFirst3::B => *self = PollFirst3::C,
            PollFirst3::C => *self = PollFirst3::A,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum PollFirst4 {
    A,
    B,
    C,
    D,
}
impl PollFirst4 {
    pub fn next(&mut self) {
        match self {
            PollFirst4::A => *self = PollFirst4::B,
            PollFirst4::B => *self = PollFirst4::C,
            PollFirst4::C => *self = PollFirst4::D,
            PollFirst4::D => *self = PollFirst4::A,
        }
    }
}

pub struct RoundRobinSelectSlice<'a, Fut> {
    inner: Pin<&'a mut [Fut]>,
    poll_next_idx: usize,
}

/// Round robin select over a slice of Unpin futures, initially starting at a
/// random value.
pub fn unbiased_select_slice<'a, Fut: Future>(
    mut rng: impl rand::Rng,
    slice: Pin<&'a mut [Fut]>,
) -> RoundRobinSelectSlice<'a, Fut> {
    // gen_range panics if range is empty, special cased below.
    let seed = if !slice.is_empty() {
        rng.gen_range(0..slice.len())
    } else {
        0
    };
    RoundRobinSelectSlice {
        poll_next_idx: seed,
        inner: slice,
    }
}

impl<'a, Fut: Future> Future for RoundRobinSelectSlice<'a, Fut> {
    type Output = (Fut::Output, usize);

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        #[inline(always)]
        fn pin_iter<T>(slice: Pin<&mut [T]>) -> impl Iterator<Item = Pin<&mut T>> {
            // Safety:
            // This is from embassy_futures::select::select_slice which refers to
            //   https://users.rust-lang.org/t/working-with-pinned-slices-are-there-any-structurally-pinning-vec-like-collection-types/50634/2
            unsafe {
                slice
                    .get_unchecked_mut()
                    .iter_mut()
                    .map(|v| Pin::new_unchecked(v))
            }
        }
        if self.inner.is_empty() {
            return Poll::Pending;
        }
        let poll_next_idx = self.poll_next_idx;
        // Panic safety: self.inner isn't empty as checked above, so won't do mod 0
        // here.
        self.poll_next_idx = (self.poll_next_idx + 1) % self.inner.len();
        for (i, fut) in pin_iter(self.inner.as_mut())
            .enumerate()
            .skip(poll_next_idx)
        {
            if let Poll::Ready(res) = fut.poll(cx) {
                return Poll::Ready((res, i));
            }
        }
        for (i, fut) in pin_iter(self.inner.as_mut())
            .enumerate()
            .take(poll_next_idx)
        {
            if let Poll::Ready(res) = fut.poll(cx) {
                return Poll::Ready((res, i));
            }
        }
        Poll::Pending
    }
}

impl<A, B> Future for RoundRobinSelect<'_, A, B>
where
    A: Future,
    B: Future,
{
    type Output = Either<A::Output, B::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let a: Pin<&mut A> = this.a;
        let b: Pin<&mut B> = this.b;
        match this.poll_first {
            PollFirst2::A => {
                this.poll_first.next();
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either::First(x));
                }
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either::Second(x));
                }
            }
            PollFirst2::B => {
                this.poll_first.next();
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either::Second(x));
                }
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either::First(x));
                }
            }
        }
        Poll::Pending
    }
}

impl<A, B, C> Future for RoundRobinSelect3<'_, A, B, C>
where
    A: Future,
    B: Future,
    C: Future,
{
    type Output = Either3<A::Output, B::Output, C::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let a: Pin<&mut A> = this.a;
        let b: Pin<&mut B> = this.b;
        let c: Pin<&mut C> = this.c;
        match this.poll_first {
            PollFirst3::A => {
                this.poll_first.next();
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either3::First(x));
                }
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either3::Second(x));
                }
                if let Poll::Ready(x) = c.poll(cx) {
                    return Poll::Ready(Either3::Third(x));
                }
            }
            PollFirst3::B => {
                this.poll_first.next();
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either3::Second(x));
                }
                if let Poll::Ready(x) = c.poll(cx) {
                    return Poll::Ready(Either3::Third(x));
                }
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either3::First(x));
                }
            }
            PollFirst3::C => {
                this.poll_first.next();
                if let Poll::Ready(x) = c.poll(cx) {
                    return Poll::Ready(Either3::Third(x));
                }
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either3::First(x));
                }
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either3::Second(x));
                }
            }
        }
        Poll::Pending
    }
}

impl<A, B, C, D> Future for RoundRobinSelect4<'_, A, B, C, D>
where
    A: Future,
    B: Future,
    C: Future,
    D: Future,
{
    type Output = Either4<A::Output, B::Output, C::Output, D::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let a = unsafe { Pin::new_unchecked(&mut this.a) };
        let b = unsafe { Pin::new_unchecked(&mut this.b) };
        let c = unsafe { Pin::new_unchecked(&mut this.c) };
        let d = unsafe { Pin::new_unchecked(&mut this.d) };
        match this.poll_first {
            PollFirst4::A => {
                this.poll_first.next();
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either4::First(x));
                }
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either4::Second(x));
                }
                if let Poll::Ready(x) = c.poll(cx) {
                    return Poll::Ready(Either4::Third(x));
                }
                if let Poll::Ready(x) = d.poll(cx) {
                    return Poll::Ready(Either4::Fourth(x));
                }
            }
            PollFirst4::B => {
                this.poll_first.next();
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either4::Second(x));
                }
                if let Poll::Ready(x) = c.poll(cx) {
                    return Poll::Ready(Either4::Third(x));
                }
                if let Poll::Ready(x) = d.poll(cx) {
                    return Poll::Ready(Either4::Fourth(x));
                }
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either4::First(x));
                }
            }
            PollFirst4::C => {
                this.poll_first.next();
                if let Poll::Ready(x) = c.poll(cx) {
                    return Poll::Ready(Either4::Third(x));
                }
                if let Poll::Ready(x) = d.poll(cx) {
                    return Poll::Ready(Either4::Fourth(x));
                }
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either4::First(x));
                }
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either4::Second(x));
                }
            }
            PollFirst4::D => {
                this.poll_first.next();
                if let Poll::Ready(x) = d.poll(cx) {
                    return Poll::Ready(Either4::Fourth(x));
                }
                if let Poll::Ready(x) = a.poll(cx) {
                    return Poll::Ready(Either4::First(x));
                }
                if let Poll::Ready(x) = b.poll(cx) {
                    return Poll::Ready(Either4::Second(x));
                }
                if let Poll::Ready(x) = c.poll(cx) {
                    return Poll::Ready(Either4::Third(x));
                }
            }
        }
        Poll::Pending
    }
}

pub fn round_robin_select<A, B>(
    poll_first: &mut PollFirst2,
    a: A,
    b: B,
) -> RoundRobinSelect<'_, A, B>
where
    A: Future,
    B: Future,
{
    RoundRobinSelect { poll_first, a, b }
}

pub fn round_robin_select3<A, B, C>(
    poll_first: &mut PollFirst3,
    a: A,
    b: B,
    c: C,
) -> RoundRobinSelect3<'_, A, B, C>
where
    A: Future,
    B: Future,
    C: Future,
{
    RoundRobinSelect3 {
        poll_first,
        a,
        b,
        c,
    }
}

pub fn round_robin_select4<A, B, C, D>(
    poll_first: &mut PollFirst4,
    a: A,
    b: B,
    c: C,
    d: D,
) -> RoundRobinSelect4<'_, A, B, C, D>
where
    A: Future,
    B: Future,
    C: Future,
    D: Future,
{
    RoundRobinSelect4 {
        poll_first,
        a,
        b,
        c,
        d,
    }
}
