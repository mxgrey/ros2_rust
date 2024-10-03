use std::{env, sync::Arc, thread};

use anyhow::{Error, Result};

use rclrs::ActionServer;

type Fibonacci = example_interfaces::action::Fibonacci;
type GoalHandleFibonacci = rclrs::ServerGoalHandle<Fibonacci>;

fn handle_goal(
    _uuid: rclrs::GoalUuid,
    goal: example_interfaces::action::Fibonacci_Goal,
) -> rclrs::GoalResponse {
    println!("Received goal request with order {}", goal.order);
    if goal.order > 9000 {
        rclrs::GoalResponse::Reject
    } else {
        rclrs::GoalResponse::AcceptAndExecute
    }
}

fn handle_cancel(_goal_handle: GoalHandleFibonacci) -> rclrs::CancelResponse {
    println!("Got request to cancel goal");
    rclrs::CancelResponse::Accept
}

fn execute(goal_handle: GoalHandleFibonacci) {
    println!("Executing goal");
    let mut feedback = example_interfaces::action::Fibonacci_Feedback {
        sequence: [0, 1].to_vec(),
    };

    for i in 1..goal_handle.goal().order {
        if goal_handle.is_canceling() {
            let result = example_interfaces::action::Fibonacci_Result {
                sequence: Vec::new(),
            };

            goal_handle.canceled(&result).ok();
            println!("Goal canceled");
            return;
        }

        // Update sequence sequence
        feedback
            .sequence
            .push(feedback.sequence[i as usize] + feedback.sequence[(i - 1) as usize]);
        // Publish feedback
        goal_handle.publish_feedback(&feedback).ok();
        println!("Publishing feedback");
        thread::sleep(std::time::Duration::from_millis(100));
    }

    let mut result = example_interfaces::action::Fibonacci_Result {
        sequence: Vec::new(),
    };
    result.sequence = feedback.sequence.clone();
    goal_handle.succeed(&result).ok();
    println!("Goal succeeded");
}

fn handle_accepted(goal_handle: GoalHandleFibonacci) {
    thread::spawn(move || {
        execute(goal_handle);
    });
}

fn main() -> Result<(), Error> {
    let context = rclrs::Context::new(env::args())?;

    let node = rclrs::create_node(&context, "minimal_action_server")?;

    let _action_server: Arc<ActionServer<example_interfaces::action::Fibonacci>> = node.create_action_server(
        "fibonacci",
        handle_goal,
        handle_cancel,
        handle_accepted,
    ).unwrap();

    rclrs::spin(node).map_err(|err| err.into())
}
