use {
	libc::{c_char, c_void},
	lightgbm_sys,
	std::{self, ffi::CString},
};

#[cfg(feature = "dataframe")]
use polars::prelude::*;

use crate::{Error, Result};

/// Dataset used throughout LightGBM for training.
///
/// # Examples
///
/// ## from mat
///
/// ```
/// use lightgbm::Dataset;
///
/// let data = &[
/// 	[1.0, 0.1, 0.2, 0.1],
/// 	[0.7, 0.4, 0.5, 0.1],
/// 	[0.9, 0.8, 0.5, 0.1],
/// 	[0.2, 0.2, 0.8, 0.7],
/// 	[0.1, 0.7, 1.0, 0.9],
/// ];
/// let label = &[0.0, 0.0, 0.0, 1.0, 1.0];
/// let dataset = Dataset::from_mat(
/// 	&data.iter().flatten().copied().collect::<Vec<_>>(),
/// 	data.len(),
/// 	label,
/// )
/// .unwrap();
/// ```
///
/// ## from file
///
/// ```
/// use lightgbm::Dataset;
///
/// let dataset =
/// 	Dataset::from_file(&"lightgbm-sys/lightgbm/examples/binary_classification/binary.train")
/// 		.unwrap();
/// ```
pub struct Dataset {
	pub(crate) handle: lightgbm_sys::DatasetHandle,
}

impl Drop for Dataset {
	fn drop(&mut self) {
		lgbm_call!(lightgbm_sys::LGBM_DatasetFree(self.handle))
			.expect("Call to LGBM_DatasetFree should always succeed");
	}
}

impl Dataset {
	fn new(handle: lightgbm_sys::DatasetHandle) -> Self {
		Self { handle }
	}

	/// Create a new `Dataset` from dense array in row-major order.
	///
	/// Example
	/// ```
	/// use lightgbm::Dataset;
	///
	/// let data = &[
	/// 	[1.0, 0.1, 0.2, 0.1],
	/// 	[0.7, 0.4, 0.5, 0.1],
	/// 	[0.9, 0.8, 0.5, 0.1],
	/// 	[0.2, 0.2, 0.8, 0.7],
	/// 	[0.1, 0.7, 1.0, 0.9],
	/// ];
	/// let label = &[0.0, 0.0, 0.0, 1.0, 1.0];
	/// let dataset = Dataset::from_mat(
	/// 	&data.iter().flatten().copied().collect::<Vec<_>>(),
	/// 	data.len(),
	/// 	label,
	/// )
	/// .unwrap();
	/// ```
	pub fn from_mat(data: &[f64], n_rows: usize, label: &[f32]) -> Result<Self> {
		let data_length = data.len();
		if (data_length != 0 || n_rows != 0) && data_length % n_rows != 0 {
			return Err(Error::new(format!(
				"data len is not multiple of n_rows ({n_rows}), but all rows \
					should have the same number of features",
			)));
		}
		let feature_length = if data_length == 0 && n_rows == 0 {
			0
		} else {
			data_length / n_rows
		};

		let nrow = n_rows
			.try_into()
			.map_err(|_| Error::new("number of rows doesn't fit into an i32"))?;
		let ncol = feature_length
			.try_into()
			.map_err(|_| Error::new("number of columns doesn't fit into an i32"))?;
		let label_len = label
			.len()
			.try_into()
			.map_err(|_| Error::new("label length doesn't fit into an i32"))?;

		let params =
			CString::new("").map_err(|e| Error::from_other("failed to make cstring", e))?;
		let label_str =
			CString::new("label").map_err(|e| Error::from_other("failed to make cstring", e))?;
		let reference = std::ptr::null_mut(); // not use
		let mut handle = std::ptr::null_mut();

		lgbm_call!(lightgbm_sys::LGBM_DatasetCreateFromMat(
			data.as_ptr() as *const c_void,
			lightgbm_sys::C_API_DTYPE_FLOAT64,
			nrow,
			ncol,
			1_i32,
			params.as_ptr() as *const c_char,
			reference,
			&mut handle
		))?;
		// It is very important to create the dataset immediately after a successful call to avoid
		// memory leak on subsequent error (as we rely on the drop impl of Dataset to be called)
		let dataset = Self::new(handle);

		lgbm_call!(lightgbm_sys::LGBM_DatasetSetField(
			handle,
			label_str.as_ptr() as *const c_char,
			label.as_ptr() as *const c_void,
			label_len,
			lightgbm_sys::C_API_DTYPE_FLOAT32
		))?;

		Ok(dataset)
	}

	/// Create a new `Dataset` from file.
	///
	/// file is `tsv`.
	/// ```text
	/// <label>\t<value>\t<value>\t...
	/// ```
	///
	/// ```text
	/// 2 0.11 0.89 0.2
	/// 3 0.39 0.1 0.4
	/// 0 0.1 0.9 1.0
	/// ```
	///
	/// Example
	/// ```
	/// use lightgbm::Dataset;
	///
	/// let dataset =
	/// 	Dataset::from_file(&"lightgbm-sys/lightgbm/examples/binary_classification/binary.train");
	/// ```
	pub fn from_file(file_path: &str) -> Result<Self> {
		let file_path_str =
			CString::new(file_path).map_err(|e| Error::from_other("failed to make cstring", e))?;
		let params =
			CString::new("").map_err(|e| Error::from_other("failed to make cstring", e))?;
		let mut handle = std::ptr::null_mut();

		lgbm_call!(lightgbm_sys::LGBM_DatasetCreateFromFile(
			file_path_str.as_ptr() as *const c_char,
			params.as_ptr() as *const c_char,
			std::ptr::null_mut(),
			&mut handle
		))?;
		// It is very important to create the dataset immediately after a successful call to avoid
		// memory leak on subsequent error (as we rely on the drop impl of Dataset to be called)
		Ok(Self::new(handle))
	}

	/// Create a new `Dataset` from a polars DataFrame.
	///
	/// Note: the feature ```dataframe``` is required for this method
	///
	/// Example
	#[cfg_attr(
		feature = "dataframe",
		doc = r##"
    extern crate polars;

    use lightgbm::Dataset;
    use polars::prelude::*;
    use polars::df;

    let df: DataFrame = df![
            "feature_1" => [1.0, 0.7, 0.9, 0.2, 0.1],
            "feature_2" => [0.1, 0.4, 0.8, 0.2, 0.7],
            "feature_3" => [0.2, 0.5, 0.5, 0.1, 0.1],
            "feature_4" => [0.1, 0.1, 0.1, 0.7, 0.9],
            "label" => [0.0, 0.0, 0.0, 1.0, 1.0]
        ].unwrap();
    let dataset = Dataset::from_dataframe(df, String::from("label")).unwrap();
    "##
	)]
	#[cfg(feature = "dataframe")]
	pub fn from_dataframe(mut dataframe: DataFrame, label_column: String) -> Result<Self> {
		let label_col_name = label_column.as_str();

		let (m, n) = dataframe.shape();

		let label_series = &dataframe.select_series(label_col_name)?[0].cast::<Float32Type>()?;

		if label_series.null_count() != 0 {
			panic!("Cannot create a dataset with null values, encountered nulls when creating the label array")
		}

		dataframe.drop_in_place(label_col_name)?;

		let mut label_values = Vec::with_capacity(m);

		let label_values_ca = label_series.unpack::<Float32Type>()?;

		label_values_ca
			.into_no_null_iter()
			.enumerate()
			.for_each(|(_row_idx, val)| {
				label_values.push(val);
			});

		let mut feature_values = Vec::with_capacity(m);
		for _i in 0..m {
			feature_values.push(Vec::with_capacity(n));
		}

		for (_col_idx, series) in dataframe.get_columns().iter().enumerate() {
			if series.null_count() != 0 {
				panic!("Cannot create a dataset with null values, encountered nulls when creating the features array")
			}

			let series = series.cast::<Float64Type>()?;
			let ca = series.unpack::<Float64Type>()?;

			ca.into_no_null_iter()
				.enumerate()
				.for_each(|(row_idx, val)| feature_values[row_idx].push(val));
		}
		Self::from_mat(feature_values, label_values)
	}

	pub fn n_rows(&self) -> Result<usize> {
		let mut result = 0_i32;
		lgbm_call!(lightgbm_sys::LGBM_DatasetGetNumData(
			self.handle,
			&mut result
		))?;
		result
			.try_into()
			.map_err(|_| Error::new("dataset length negative"))
	}

	pub fn n_features(&self) -> Result<usize> {
		let mut result = 0_i32;
		lgbm_call!(lightgbm_sys::LGBM_DatasetGetNumFeature(
			self.handle,
			&mut result
		))?;
		result
			.try_into()
			.map_err(|_| Error::new("feature count negative"))
	}

	pub fn set_weights(&mut self, weights: &[f32]) -> Result<()> {
		let n_rows = self.n_rows()?;
		if n_rows != weights.len() {
			return Err(Error::new(format!(
				"got {} weights, but dataset has {} records",
				weights.len(),
				n_rows
			)));
		}
		let field_name = CString::new("weight").unwrap();
		let len = weights
			.len()
			.try_into()
			.map_err(|_| Error::new("weights len doesn't fit into an i32"))?;
		lgbm_call!(lightgbm_sys::LGBM_DatasetSetField(
			self.handle,
			field_name.as_ptr() as *const c_char,
			weights.as_ptr() as *const c_void,
			len,
			lightgbm_sys::C_API_DTYPE_FLOAT32 as i32,
		))?;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	fn read_train_file() -> Result<Dataset> {
		Dataset::from_file("lightgbm-sys/lightgbm/examples/binary_classification/binary.train")
	}

	#[test]
	fn read_file() {
		assert!(read_train_file().is_ok());
	}

	#[test]
	fn from_mat() {
		let data = &[
			[1.0, 0.1, 0.2, 0.1],
			[0.7, 0.4, 0.5, 0.1],
			[0.9, 0.8, 0.5, 0.1],
			[0.2, 0.2, 0.8, 0.7],
			[0.1, 0.7, 1.0, 0.9],
		];
		let label = &[0.0, 0.0, 0.0, 1.0, 1.0];
		let dataset = Dataset::from_mat(
			&data.iter().flatten().copied().collect::<Vec<_>>(),
			data.len(),
			label,
		);
		assert!(dataset.is_ok());
	}

	#[cfg(feature = "dataframe")]
	#[test]
	fn from_dataframe() {
		use polars::df;
		let df: DataFrame = df![
			"feature_1" => [1.0, 0.7, 0.9, 0.2, 0.1],
			"feature_2" => [0.1, 0.4, 0.8, 0.2, 0.7],
			"feature_3" => [0.2, 0.5, 0.5, 0.1, 0.1],
			"feature_4" => [0.1, 0.1, 0.1, 0.7, 0.9],
			"label" => [0.0, 0.0, 0.0, 1.0, 1.0]
		]
		.unwrap();

		let df_dataset = Dataset::from_dataframe(df, String::from("label"));
		assert!(df_dataset.is_ok());
	}

	#[test]
	fn get_dataset_properties() {
		let data = &[
			[1.0, 0.1, 0.2, 0.1],
			[0.7, 0.4, 0.5, 0.1],
			[0.9, 0.8, 0.5, 0.1],
			[0.2, 0.2, 0.8, 0.7],
			[0.1, 0.7, 1.0, 0.9],
		];
		let label = &[0.0, 0.0, 0.0, 1.0, 1.0];
		let dataset = Dataset::from_mat(
			&data.iter().flatten().copied().collect::<Vec<_>>(),
			data.len(),
			label,
		)
		.unwrap();
		assert_eq!(dataset.n_rows(), Ok(5));
		assert_eq!(dataset.n_features(), Ok(4));
	}

	#[test]
	fn set_weights() {
		let data = &[
			[1.0, 0.1, 0.2, 0.1],
			[0.7, 0.4, 0.5, 0.1],
			[0.9, 0.8, 0.5, 0.1],
			[0.2, 0.2, 0.8, 0.7],
			[0.1, 0.7, 1.0, 0.9],
		];
		let label = &[0.0, 0.0, 0.0, 1.0, 1.0];
		let mut dataset = Dataset::from_mat(
			&data.iter().flatten().copied().collect::<Vec<_>>(),
			data.len(),
			label,
		)
		.unwrap();
		let weights = &[0.5, 1.0, 2.0, 0.5, 0.5];
		dataset.set_weights(weights).unwrap();
	}

	#[test]
	fn set_weights_wrong_len() {
		let data = &[
			[1.0, 0.1, 0.2, 0.1],
			[0.7, 0.4, 0.5, 0.1],
			[0.9, 0.8, 0.5, 0.1],
			[0.2, 0.2, 0.8, 0.7],
			[0.1, 0.7, 1.0, 0.9],
		];
		let label = &[0.0, 0.0, 0.0, 1.0, 1.0];
		let mut dataset = Dataset::from_mat(
			&data.iter().flatten().copied().collect::<Vec<_>>(),
			data.len(),
			label,
		)
		.unwrap();
		let weights_short = &[0.5, 1.0, 2.0, 0.5];
		let weights_long = &[0.5, 1.0, 2.0, 0.5, 0.1, 0.1];
		assert!(dataset.set_weights(weights_short).is_err());
		assert!(dataset.set_weights(weights_long).is_err());
	}
}
