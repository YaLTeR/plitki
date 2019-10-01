#[doc(hidden)]
#[macro_export]
macro_rules! impl_ops {
    ($type:ty, $type_difference:ty) => {
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
                Self::Output {
                    0: self.0 + other.0,
                }
            }
        }

        impl Sub<$type_difference> for $type {
            type Output = $type;

            #[inline]
            fn sub(self, other: $type_difference) -> Self::Output {
                Self::Output {
                    0: self.0 - other.0,
                }
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
    };
}
