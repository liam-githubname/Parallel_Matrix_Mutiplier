/*
* Author: Liam Harvell
* UCFID: 5507384
*/
use std::env;
use std::fs;
use std::process;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
#[derive(Default, Debug)]
struct Config {
    file_path: String,
    matrix_one: Matrix,
    matrix_two: Matrix,
}
#[derive(Default, Debug)]
struct Matrix {
    num_of_rows: i32,
    num_of_cols: i32,
    elements: Vec<Vec<i32>>,
}

fn main() {
    let timer = Instant::now();
    let args: Vec<String> = env::args().collect();
    match Config::build(&args) {
        Ok(config) => {
            let result = multiply_matrices(config.matrix_one, config.matrix_two);
            result.print("Resultant Matrix");
        }
        Err(err) => {
            println!("Error processing file: {err}");
            process::exit(1);
        }
    };
    println!("\n{:?}", timer.elapsed());
}

fn multiply_matrices(matrix1: Matrix, matrix2: Matrix) -> Matrix {
    //Input Validation
    if matrix1.num_of_cols != matrix2.num_of_rows {
        println!("Invalid Input");
        process::exit(1);
    }

    //Prepare the resultant matrix
    // NOTE: This cannot be after matrix#_arc borrows matrix1 and matrix2
    // Diagnostics:
    // 1. use of moved value: `matrix1`
    //   value used here after move [E0382]*/
    let mut resultant_matrix = Matrix {
        num_of_rows: matrix1.num_of_rows,
        num_of_cols: matrix2.num_of_cols,
        elements: Vec::new(),
    };
    for _ in 0..resultant_matrix.num_of_rows {
        resultant_matrix
            .elements
            .push(vec![0; resultant_matrix.num_of_cols as usize]);
    }

    //Manage threads
    let matrix1_arc = Arc::new(matrix1);
    let matrix2_arc = Arc::new(matrix2);

    // NOTE: thread::available_parallelism() returns 12 on my machine
    // NOTE: available_parallelism() -> NonZero<usize> but we need number_of_cores to be a usize
    let number_of_cores = thread::available_parallelism()
        // NOTE: .map(|num| num.get()) maps a function to the Ok() value, this leaves Err() value
        //untouched
        .map(|num| num.get())
        // NOTE: .unwrap_or() returns the Ok() value or a default value, right now it is 1 idk that should be
        //changed
        .unwrap_or(1);

    // NOTE: I don't know if two threads per core is good or not just what was suggested
    let number_of_threads = number_of_cores * 2;
    // NOTE: "+ number_of_threads - 1" ensures that rows_per_thread will not be too small to contain all
    //rows
    let rows_per_thread =
        (matrix1_arc.num_of_rows as usize + number_of_threads - 1) / number_of_threads;

    //Share data between Threads
    // NOTE: channel creates an asynchronous and returns the sender and receiver halves sender can
    //and will be copied for each thread, however the receiver can only have one instance
    let (sender, receiver) = mpsc::channel();
    //This is created to save references(JoinHandles) to thread handles
    let mut handles = vec![];

    //Thread Coordination
    for thread_id in 0..number_of_threads {
        // NOTE: sender is being copied per thread, all clones must be closed for the receiver to be
        //closed
        let sender = sender.clone();
        // each thread (thread_id) is given it's rows to work on here
        let start_row = thread_id * rows_per_thread;
        let end_row = (start_row + rows_per_thread).min(resultant_matrix.num_of_rows as usize);

        if start_row >= resultant_matrix.num_of_rows as usize {
            continue;
        }

        // NOTE: Makes a clone of the `Arc` pointer. This creates another pointer to the same allocation, increasing the strong reference count.
        let matrix1_ref = Arc::clone(&matrix1_arc);
        let matrix2_ref = Arc::clone(&matrix2_arc);

        //Distributes the workload between threads
        // NOTE: spawn() takes a closure, with contraints because threads can outlive their caller. The contraints have a 'static lifetime
        // because the return can outlive the caller
        // NOTE: Capture a [closure](https://doc.rust-lang.org/stable/book/ch13-01-closures.html)'s
        // environment by value. `move` converts any variables captured by reference or mutable reference to variables captured by value.
        let handle = thread::spawn(move || {
            let mut results = Vec::new();

            // NOTE: .iter() creates an iterator for the outer Vec<Vec<i32>> and .enumerate()
            // returns an iterator that contains the current index and the value at the current
            // index
            for (local_row_idx, row) in matrix1_ref.elements[start_row..end_row].iter().enumerate()
            {
                let mut result_row = vec![0; resultant_matrix.num_of_cols as usize];
                for j in 0..resultant_matrix.num_of_cols as usize {
                    let mut sum = 0;
                    for k in 0..row.len() {
                        // NOTE: row is matrix1_ref.elements[current row]
                        sum += row[k] * matrix2_ref.elements[k][j];
                    }
                    result_row[j] = sum;
                }
                results.push((start_row + local_row_idx, result_row));
            }
            // NOTE: .send(results) attempts to send the results: Vec<i32> on the channel created
            // when mpsc::channel() was called returns a Result<(), SendError<T>>, but in this
            // context I don't know if it needs to be unwrapped if I'm not going to handle the potential
            // error

            //Send results back to main thread (through receiver)
            sender.send(results).unwrap();
        });
        handles.push(handle);
    }
    // NOTE: This must be done otherwise the receiver wouldn't close
    drop(sender);

    // TODO: result aggregation
    while let Ok(thread_results) = receiver.recv() {
        for (row_idx, row) in thread_results {
            resultant_matrix.elements[row_idx] = row;
        }
    }
    //Rejoins the threads
    for handle in handles {
        handle.join().unwrap();
    }
    resultant_matrix
}

impl Config {
    fn get_matrices_from_file(file_path: &String) -> (Matrix, Matrix) {
        let content = fs::read_to_string(file_path).unwrap();
        let mut parsed_integers_from_input = Vec::new();
        for line in content.lines() {
            for number_str in line.split_whitespace() {
                match number_str.parse::<i32>() {
                    Ok(number) => parsed_integers_from_input.push(number),
                    Err(e) => println!("Error parsing number {}: {}", number_str, e),
                }
            }
        }
        let mut matrix1 = Matrix::default();
        let mut matrix2 = Matrix::default();
        let mut index = 2;
        matrix1.num_of_rows = parsed_integers_from_input[0];
        matrix1.num_of_cols = parsed_integers_from_input[1];
        for _ in 0..matrix1.num_of_rows {
            let mut current_row = Vec::new();
            for _ in 0..matrix1.num_of_cols {
                current_row.push(parsed_integers_from_input[index]);
                index += 1;
            }
            matrix1.elements.push(current_row);
        }
        matrix2.num_of_rows = parsed_integers_from_input[index];
        matrix2.num_of_cols = parsed_integers_from_input[index + 1];
        index += 2;
        for _ in 0..matrix2.num_of_rows {
            let mut current_row = Vec::new();
            for _ in 0..matrix2.num_of_cols {
                current_row.push(parsed_integers_from_input[index]);
                index += 1;
            }
            matrix2.elements.push(current_row);
        }
        (matrix1, matrix2)
    }

    fn build(args: &[String]) -> Result<Config, &'static str> {
        if args.len() < 2 {
            return Err("not enough arguments");
        }
        let file_path = args[1].clone();
        let matrices = Config::get_matrices_from_file(&file_path);
        Ok(Config {
            file_path: file_path,
            matrix_one: matrices.0,
            matrix_two: matrices.1,
        })
    }
}

impl Matrix {
    fn print(self, title: &str) {
        println!("{title}:");
        for vec in self.elements {
            print!("    ");
            for i in 0..vec.len() {
                print!("{} ", vec[i]);
            }
            println!();
        }
    }
}
