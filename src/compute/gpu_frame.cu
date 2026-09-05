/*GPU Frame Header*/
#include "compute/gpu_frame.cuh"

/*CUDA Custom Registration Namespace (Compiling as DLL)*/
namespace gpu_cost_function {

/*If successful, uploads the four host images
else, marked as not initialized correctly*/
GPUFrame::GPUFrame(
    int width,
    int height,
    int gpu_device,
    unsigned char* host_image) {
    /*Try Initializing GPU Images First*/
    gpu_image_ = std::make_unique<GPUImage>(width, height, gpu_device, host_image);

    /*If Successful*/
    if (gpu_image_->IsInitializedCorrectly()) {
        height_ = height;
        width_ = width;
        initialized_correctly_ = true;
    } else {
        height_ = 0;
        width_ = 0;
        initialized_correctly_ = false;
    }
};
void GPUFrame::WriteGPUImage(std::string file_name) {
    GetGPUImage().WriteImage(file_name);
}
/*Default constructor. Marked as not initialized correctly.*/
GPUFrame::GPUFrame() {
    height_ = 0;
    width_ = 0;
    initialized_correctly_ = false;

    /*GPU image ownership defaults to null (unique_ptr); it is populated only
     * by the parameterized constructor*/
};

/*Default Destructor*/
GPUFrame::~GPUFrame() {
};

/*Get pointer to the image on the GPU Images */
unsigned char* GPUFrame::GetDeviceImagePointer() {
    return gpu_image_->GetDeviceImagePointer();
};

/*Get pointer to the actual GPU Image*/
GPUImage& GPUFrame::GetGPUImage() {
    return *gpu_image_;
};

/*Get Image Size Parameters*/
int GPUFrame::GetFrameHeight() const {
    return height_;
};

int GPUFrame::GetFrameWidth() const {
    return width_;
};

/*Get Is Initialized Correctly*/
bool GPUFrame::IsInitializedCorrectly() {
    return initialized_correctly_;
};

}  // namespace gpu_cost_function
