#[doc(hidden)]
#[macro_export]
macro_rules! impl_ops {
    ($type:ty, $type_into:ident, $type_checked_from:ident, $type_saturating_from:ident, $type_difference:ty, $type_difference_into:ident) => {
        impl Sub<$type> for $type {
            type Output = $type_difference;

            #[inline]
            fn sub(self, other: $type) -> Self::Output {
                Self::Output {
                    0: self.0 - other.0,
                }
            }
        }

        impl Add<$type_difference> for $type {
            type Output = $type;

            #[inline]
            fn add(self, other: $type_difference) -> Self::Output {
                Self::Output::$type_checked_from(self.$type_into() + other.$type_difference_into())
                    .unwrap()
            }
        }

        impl Sub<$type_difference> for $type {
            type Output = $type;

            #[inline]
            fn sub(self, other: $type_difference) -> Self::Output {
                Self::Output::$type_checked_from(self.$type_into() - other.$type_difference_into())
                    .unwrap()
            }
        }

        impl Add<$type> for $type_difference {
            type Output = $type;

            #[inline]
            fn add(self, other: $type) -> Self::Output {
                other + self
            }
        }

        impl Add<$type_difference> for $type_difference {
            type Output = $type_difference;

            #[inline]
            fn add(self, other: $type_difference) -> Self::Output {
                Self::Output {
                    0: self.0 + other.0,
                }
            }
        }

        impl Sub<$type_difference> for $type_difference {
            type Output = $type_difference;

            #[inline]
            fn sub(self, other: $type_difference) -> Self::Output {
                Self::Output {
                    0: self.0 - other.0,
                }
            }
        }

        impl $type {
            /// Saturating subtraction. Computes `self - rhs`, saturating at the numeric bounds
            /// instead of overflowing.
            #[inline]
            pub fn saturating_sub(self, other: $type_difference) -> $type {
                type Output = $type;
                Output::$type_saturating_from(
                    self.$type_into()
                        .saturating_sub(other.$type_difference_into()),
                )
            }

            /// Saturating addition. Computes `self + rhs`, saturating at the numeric bounds instead
            /// of overflowing.
            #[inline]
            pub fn saturating_add(self, other: $type_difference) -> $type {
                type Output = $type;
                Output::$type_saturating_from(
                    self.$type_into()
                        .saturating_add(other.$type_difference_into()),
                )
            }
        }

        impl $type_difference {
            /// Saturating subtraction. Computes `self - rhs`, saturating at the numeric bounds
            /// instead of overflowing.
            #[inline]
            pub fn saturating_sub(self, other: $type_difference) -> $type_difference {
                type Output = $type_difference;
                Output {
                    0: self.0.saturating_sub(other.0),
                }
            }

            /// Saturating addition. Computes `self + rhs`, saturating at the numeric bounds instead
            /// of overflowing.
            #[inline]
            pub fn saturating_add(self, other: $type_difference) -> $type_difference {
                type Output = $type_difference;
                Output {
                    0: self.0.saturating_add(other.0),
                }
            }
        }
    };
}
