use color_eyre::Result;
use tensorflow::eager::{self, raw_ops, ToTensorHandle};
use tensorflow::{Graph, Operation, SavedModelBundle, SessionOptions, Tensor};
use tensorflow::{SessionRunArgs, DEFAULT_SERVING_SIGNATURE_DEF_KEY};

// Python version: https://github.com/GantMan/nsfw_model/blob/master/nsfw_detector/predict.py
// C++ Basis for similar thing: https://github.com/tensorflow/rust/blob/master/examples/mobilenetv3.rs

pub fn predict() -> Result<()> {
    let opts = eager::ContextOptions::new();
    let ctx = eager::Context::new(opts)?;

    let fname = "nude-test-1.jpg".to_handle(&ctx)?;
    let buf = raw_ops::read_file(&ctx, &fname)?;
    let img = raw_ops::decode_image(&ctx, &buf)?;
    let cast2float = raw_ops::Cast::new().DstT(tensorflow::DataType::Float);
    let img = cast2float.call(&ctx, &img)?;
    let batch = raw_ops::expand_dims(&ctx, &img, &0)?; // add batch dim
    let readonly_x = batch.resolve()?;

    // The current eager API implementation requires unsafe block to feed the tensor into a graph.
    let x: Tensor<f32> = unsafe { readonly_x.into_tensor() };

    // Load the model.
    let mut graph = Graph::new();
    let bundle = SavedModelBundle::load(
        &SessionOptions::new(),
        &["train", "serve"],
        &mut graph,
        "export",
    )?;
    let session = &bundle.session;

    // get in/out operations
    println!(
        "Signatures: {:?}",
        bundle.meta_graph_def().signatures().keys()
    );
    let signature = bundle
        .meta_graph_def()
        .get_signature(DEFAULT_SERVING_SIGNATURE_DEF_KEY)?;
    let x_info = signature.get_input("input_1")?;
    let op_x = &graph.operation_by_name_required(&x_info.name().name)?;
    let output_info = signature.get_output("Predictions")?;
    let op_output = &graph.operation_by_name_required(&output_info.name().name)?;

    // Run the graph.
    let mut args = SessionRunArgs::new();
    args.add_feed(op_x, 0, &x);
    let token_output = args.request_fetch(op_output, 0);
    session.run(&mut args)?;

    // Check the output.
    let output: Tensor<f32> = args.fetch(token_output)?;

    // Calculate argmax of the output
    let (max_idx, _max_val) =
        output
            .iter()
            .enumerate()
            .fold((0, output[0]), |(idx_max, val_max), (idx, val)| {
                if &val_max > val {
                    (idx_max, val_max)
                } else {
                    (idx, *val)
                }
            });

    // This index is expected to be identical with that of the Python code,
    // but this is not guaranteed due to floating operations.
    println!("argmax={}", max_idx);

    Ok(())
}

fn main() -> Result<()> {
    color_eyre::install()?;

    predict()?;
    Ok(())
}
