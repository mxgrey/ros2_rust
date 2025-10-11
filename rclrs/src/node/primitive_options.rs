use crate::{
    QoSDurabilityPolicy, QoSDuration, QoSHistoryPolicy, QoSLivelinessPolicy, QoSProfile,
    QoSReliabilityPolicy,
};

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

#[cfg(feature = "schemars")]
use schemars::JsonSchema;

use std::{
    borrow::{Borrow, Cow},
    time::Duration,
};

/// `PrimitiveOptions` are the subset of options that are relevant across all
/// primitives (e.g. [`Subscription`][1], [`Publisher`][2], [`Client`][3], and
/// [`Service`][4]).
///
/// Each different primitive type may have its own defaults for the overall
/// quality of service settings, and we cannot know what the default will be
/// until the `PrimitiveOptions` gets converted into the more specific set of
/// options. Therefore we store each quality of service field separately so that
/// we will only override the settings that the user explicitly asked for, and
/// the rest will be determined by the default settings for each primitive.
///
/// [1]: crate::Subscription
/// [2]: crate::Publisher
/// [3]: crate::Client
/// [4]: crate::Service
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PrimitiveOptions<'a> {
    /// The name that will be used for the primitive
    pub name: Cow<'a, str>,
    /// Options related to quality of service
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub qos: QosOptions,
}

impl<'a> PrimitiveOptions<'a> {
    /// Begin building a new set of `PrimitiveOptions` with only the name set.
    pub fn new(name: impl Into<Cow<'a, str>>) -> Self {
        Self {
            name: name.into(),
            qos: Default::default(),
        }
    }
}

/// `QosOptions` contains the subset of `PrimitiveOptions` specific to quality
/// of service settings. Each field in this struct is an `Option` to track
/// whether it has been explicitly overridden by a user option. When these
/// options get applied to a specific primitive type, it will use the default
/// for that specific primitive type if the user did not explicitly set it.
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct QosOptions {
    /// Override the default [`QoSProfile::history`] for the primitive.
    pub history: Option<QoSHistoryPolicy>,
    /// Override the default [`QoSProfile::reliability`] for the primitive.
    pub reliability: Option<QoSReliabilityPolicy>,
    /// Override the default [`QoSProfile::durability`] for the primitive.
    pub durability: Option<QoSDurabilityPolicy>,
    /// Override the default [`QoSProfile::deadline`] for the primitive.
    pub deadline: Option<QoSDuration>,
    /// Override the default [`QoSProfile::lifespan`] for the primitive.
    pub lifespan: Option<QoSDuration>,
    /// Override the default [`QoSProfile::liveliness`] for the primitive.
    pub liveliness: Option<QoSLivelinessPolicy>,
    /// Override the default [`QoSProfile::liveliness_lease`] for the primitive.
    pub liveliness_lease: Option<QoSDuration>,
    /// Override the default [`QoSProfile::avoid_ros_namespace_conventions`] for the primitive.
    pub avoid_ros_namespace_conventions: Option<bool>,
}

impl QosOptions {
    /// Apply the user-specified options to a pre-initialized [`QoSProfile`].
    pub fn apply_to(&self, qos: &mut QoSProfile) {
        if let Some(history) = self.history {
            qos.history = history;
        }

        if let Some(reliability) = self.reliability {
            qos.reliability = reliability;
        }

        if let Some(durability) = self.durability {
            qos.durability = durability;
        }

        if let Some(deadline) = self.deadline {
            qos.deadline = deadline;
        }

        if let Some(lifespan) = self.lifespan {
            qos.lifespan = lifespan;
        }

        if let Some(liveliness) = self.liveliness {
            qos.liveliness = liveliness;
        }

        if let Some(liveliness_lease) = self.liveliness_lease {
            qos.liveliness_lease = liveliness_lease;
        }

        if let Some(convention) = self.avoid_ros_namespace_conventions {
            qos.avoid_ros_namespace_conventions = convention;
        }
    }
}

/// Trait to implicitly convert a compatible object into [`PrimitiveOptions`].
pub trait IntoPrimitiveOptions<'a>: Sized {
    /// Convert the object into [`PrimitiveOptions`] with default settings.
    fn into_primitive_options(self) -> PrimitiveOptions<'a>;

    /// Override all the quality of service settings for the primitive.
    fn qos(self, profile: QoSProfile) -> PrimitiveOptions<'a> {
        let mut options = self
            .into_primitive_options()
            .history(profile.history)
            .reliability(profile.reliability)
            .durability(profile.durability)
            .deadline(profile.deadline)
            .lifespan(profile.lifespan)
            .liveliness(profile.liveliness)
            .liveliness_lease(profile.liveliness_lease);

        if profile.avoid_ros_namespace_conventions {
            options.qos.avoid_ros_namespace_conventions = Some(true);
        }

        options
    }

    /// Use the default topics quality of service profile.
    fn topics_qos(self) -> PrimitiveOptions<'a> {
        self.qos(QoSProfile::topics_default())
    }

    /// Use the default sensor data quality of service profile.
    fn sensor_data_qos(self) -> PrimitiveOptions<'a> {
        self.qos(QoSProfile::sensor_data_default())
    }

    /// Use the default services quality of service profile.
    fn services_qos(self) -> PrimitiveOptions<'a> {
        self.qos(QoSProfile::services_default())
    }

    /// Use the system-defined default quality of service profile. Topics and
    /// services created with this default do not use the recommended ROS
    /// defaults; they will instead use the default as defined by the underlying
    /// RMW implementation (rmw_fastrtps, rmw_connextdds, etc). These defaults
    /// may not always be appropriate for every use-case, and may be different
    /// depending on which RMW implementation you are using, so use caution!
    fn system_qos(self) -> PrimitiveOptions<'a> {
        self.qos(QoSProfile::system_default())
    }

    /// Override the default [`QoSProfile::history`] for the primitive.
    fn history(self, history: QoSHistoryPolicy) -> PrimitiveOptions<'a> {
        let mut options = self.into_primitive_options();
        options.qos.history = Some(history);
        options
    }

    /// Keep the last `depth` messages for the primitive.
    fn keep_last(self, depth: u32) -> PrimitiveOptions<'a> {
        self.history(QoSHistoryPolicy::KeepLast { depth })
    }

    /// Keep all messages for the primitive.
    fn keep_all(self) -> PrimitiveOptions<'a> {
        self.history(QoSHistoryPolicy::KeepAll)
    }

    /// Override the default [`QoSProfile::reliability`] for the primitive.
    fn reliability(self, reliability: QoSReliabilityPolicy) -> PrimitiveOptions<'a> {
        let mut options = self.into_primitive_options();
        options.qos.reliability = Some(reliability);
        options
    }

    /// Set the primitive to have [reliable][QoSReliabilityPolicy::Reliable] communication.
    fn reliable(self) -> PrimitiveOptions<'a> {
        self.reliability(QoSReliabilityPolicy::Reliable)
    }

    /// Set the primitive to have [best-effort][QoSReliabilityPolicy::BestEffort] communication.
    fn best_effort(self) -> PrimitiveOptions<'a> {
        self.reliability(QoSReliabilityPolicy::BestEffort)
    }

    /// Override the default [`QoSProfile::durability`] for the primitive.
    fn durability(self, durability: QoSDurabilityPolicy) -> PrimitiveOptions<'a> {
        let mut options = self.into_primitive_options();
        options.qos.durability = Some(durability);
        options
    }

    /// Set the primitive to have [volatile][QoSDurabilityPolicy::Volatile] durability.
    fn volatile(self) -> PrimitiveOptions<'a> {
        self.durability(QoSDurabilityPolicy::Volatile)
    }

    /// Set the primitive to have [transient local][QoSDurabilityPolicy::TransientLocal] durability.
    fn transient_local(self) -> PrimitiveOptions<'a> {
        self.durability(QoSDurabilityPolicy::TransientLocal)
    }

    /// Override the default [`QoSProfile::lifespan`] for the primitive.
    fn lifespan(self, lifespan: QoSDuration) -> PrimitiveOptions<'a> {
        let mut options = self.into_primitive_options();
        options.qos.lifespan = Some(lifespan);
        options
    }

    /// Set a custom duration for the [lifespan][QoSProfile::lifespan] of the primitive.
    fn lifespan_duration(self, duration: Duration) -> PrimitiveOptions<'a> {
        self.lifespan(QoSDuration::Custom(duration))
    }

    /// Make the [lifespan][QoSProfile::lifespan] of the primitive infinite.
    fn infinite_lifespan(self) -> PrimitiveOptions<'a> {
        self.lifespan(QoSDuration::Infinite)
    }

    /// Override the default [`QoSProfile::deadline`] for the primitive.
    fn deadline(self, deadline: QoSDuration) -> PrimitiveOptions<'a> {
        let mut options = self.into_primitive_options();
        options.qos.deadline = Some(deadline);
        options
    }

    /// Set the [`QoSProfile::deadline`] to a custom finite value.
    fn deadline_duration(self, duration: Duration) -> PrimitiveOptions<'a> {
        self.deadline(QoSDuration::Custom(duration))
    }

    /// Do not use a deadline for liveliness for this primitive.
    fn no_deadline(self) -> PrimitiveOptions<'a> {
        self.deadline(QoSDuration::Infinite)
    }

    /// Override the default [`QoSProfile::liveliness`] for the primitive.
    fn liveliness(self, liveliness: QoSLivelinessPolicy) -> PrimitiveOptions<'a> {
        let mut options = self.into_primitive_options();
        options.qos.liveliness = Some(liveliness);
        options
    }

    /// Set liveliness to [`QoSLivelinessPolicy::Automatic`].
    fn liveliness_automatic(self) -> PrimitiveOptions<'a> {
        self.liveliness(QoSLivelinessPolicy::Automatic)
    }

    /// Set liveliness to [`QoSLivelinessPolicy::ManualByTopic`]
    fn liveliness_manual(self) -> PrimitiveOptions<'a> {
        self.liveliness(QoSLivelinessPolicy::ManualByTopic)
    }

    /// Override the default [`QoSProfile::liveliness_lease`] for the primitive.
    fn liveliness_lease(self, lease: QoSDuration) -> PrimitiveOptions<'a> {
        let mut options = self.into_primitive_options();
        options.qos.liveliness_lease = Some(lease);
        options
    }

    /// Set a custom duration for the [liveliness lease][QoSProfile::liveliness_lease].
    fn liveliness_lease_duration(self, duration: Duration) -> PrimitiveOptions<'a> {
        self.liveliness_lease(QoSDuration::Custom(duration))
    }

    /// [Avoid the ROS namespace conventions][1] for the primitive.
    ///
    /// [1]: QoSProfile::avoid_ros_namespace_conventions
    fn avoid_ros_namespace_conventions(self) -> PrimitiveOptions<'a> {
        let mut options = self.into_primitive_options();
        options.qos.avoid_ros_namespace_conventions = Some(true);
        options
    }
}

impl<'a> IntoPrimitiveOptions<'a> for PrimitiveOptions<'a> {
    fn into_primitive_options(self) -> PrimitiveOptions<'a> {
        self
    }
}

impl<'a> IntoPrimitiveOptions<'a> for &'a str {
    fn into_primitive_options(self) -> PrimitiveOptions<'a> {
        PrimitiveOptions::new(self)
    }
}

impl<'a, T: Borrow<str>> IntoPrimitiveOptions<'a> for &'a T {
    fn into_primitive_options(self) -> PrimitiveOptions<'a> {
        self.borrow().into_primitive_options()
    }
}
