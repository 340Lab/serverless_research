use crate::{
    sim_env::SimEnv,
    fn_dag::FnId,
    algos::ContainerMetric,
    scale_executor::{ ScaleExecutor, ScaleOption },
    es::ESScaler,
};

pub struct FnScheScaler {}

impl FnScheScaler {
    pub fn new() -> Self {
        Self {}
    }
}

impl ESScaler for FnScheScaler {
    fn scale_for_fn(&mut self, env: &SimEnv, fnid: FnId, metric: &ContainerMetric) {
        // 对于容器一段时间未使用，就执行缩减
        // 优先扩容到索引小的node上
        let mut containers_2_zero = vec![];
        if let Some(nodes) = env.fn_2_nodes.borrow().get(&fnid) {
            for &nodeid in nodes.iter() {
                let node = env.node(nodeid);
                let container = node.fn_containers.get(&fnid).unwrap();

                if container.recent_frame_is_idle(3) && container.req_fn_state.len() == 0 {
                    containers_2_zero.push((fnid, nodeid));
                    log::info!("scale down fn {} on node {}", fnid, nodeid);
                } else {
                    // log::info!(
                    //     "keep fn {} on node {} left task {} working_rec {:?}",
                    //     fnid,
                    //     nodeid,
                    //     container.req_fn_state.len(),
                    //     container.recent_frames_working_cnt
                    // );
                }
            }
        }
        containers_2_zero.iter().for_each(|&(fnid, nodeid)| {
            env.scale_executor
                .borrow_mut()
                .scale_down(env, ScaleOption::ForSpecNodeFn(nodeid, fnid));
        });

        for &req_id in &metric.ready_2_schedule_fn_reqs {
            // 寻找一个有空间的node进行调度
            let mut found_node = None;
            for n in env.nodes.borrow_mut().iter_mut() {
                if n.fn_containers.get_mut(&fnid).is_some() {
                    if n.left_mem() / ((n.task_cnt() + 1) as f32) < env.func(fnid).mem {
                        continue;
                    }
                }
                found_node = Some(n.node_id());
                break;
            }
            if let Some(found_node) = found_node {
                if env.node(found_node).container(fnid).is_none() {
                    env.scale_executor
                        .borrow_mut()
                        .scale_up_fn_to_nodes(env, fnid, &vec![found_node]);
                }
                let mut req = env.request_mut(req_id);
                env.schedule_reqfn_on_node(&mut *req, fnid, found_node);
            }
            // log::info!("schedule req {} to node {}", req_id, found_node);
        }
    }
}