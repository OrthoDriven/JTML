#ifndef GPU_ARENA
#define GPU_ARENA

class GPUArena {
public:
    explicit GPUArena();

    ~GPUArena();

private:
    DeviceBuffer<Triangle> triangles;
}

#endif  // GPU_ARENA
