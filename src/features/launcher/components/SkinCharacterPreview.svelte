<script lang="ts">
  import { onMount } from "svelte";
  import {
    AmbientLight,
    Box3,
    DirectionalLight,
    FrontSide,
    Group,
    MathUtils,
    Mesh,
    PerspectiveCamera,
    Scene,
    SRGBColorSpace,
    Texture,
    TextureLoader,
    Vector3,
    WebGLRenderer,
    type Material,
    NearestFilter,
  } from "three";
  import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
  import steveModelUrl from "../../../assets/skins/default/steve.glb?url";
  import alexModelUrl from "../../../assets/skins/default/alex.glb?url";

  type ArmType = "wide" | "slim";

  export let skinUrl = "";
  export let armType: ArmType = "wide";
  export let interactive = true;
  export let scale = 1;
  export let initialYaw = 180;

  const gltfLoader = new GLTFLoader();
  const textureLoader = new TextureLoader();

  let hostElement: HTMLDivElement;
  let scene: Scene | null = null;
  let camera: PerspectiveCamera | null = null;
  let renderer: WebGLRenderer | null = null;
  let pivotGroup: Group | null = null;
  let modelGroup: Group | null = null;
  let resizeObserver: ResizeObserver | null = null;

  let modelSize = new Vector3(1, 2, 1);
  let initialized = false;
  let currentModelArmType: ArmType | null = null;
  let currentSkinSource = "";
  let currentSkinTexture: Texture | null = null;

  let modelRequestId = 0;
  let skinRequestId = 0;

  let yaw = initialYaw;
  let lastInitialYaw = initialYaw;
  let activePointerId: number | null = null;
  let dragStartX = 0;
  let dragStartYaw = 0;

  function setupRenderer(): void {
    scene = new Scene();

    camera = new PerspectiveCamera(35, 1, 0.01, 1000);

    renderer = new WebGLRenderer({
      alpha: true,
      antialias: true,
      powerPreference: "high-performance",
    });
    renderer.outputColorSpace = SRGBColorSpace;
    renderer.setClearAlpha(0);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.domElement.className = "skin-preview-canvas";

    hostElement.appendChild(renderer.domElement);

    const ambientLight = new AmbientLight(0xffffff, 1.25);
    const keyLight = new DirectionalLight(0xffffff, 1.15);
    keyLight.position.set(1.7, 2.4, 2.9);

    const fillLight = new DirectionalLight(0x9db9ff, 0.42);
    fillLight.position.set(-2.2, 0.8, -1.6);

    scene.add(ambientLight, keyLight, fillLight);

    pivotGroup = new Group();
    scene.add(pivotGroup);
  }

  function setNearestFiltering(texture: Texture): void {
    texture.colorSpace = SRGBColorSpace;
    texture.flipY = false;
    texture.generateMipmaps = false;
    texture.magFilter = NearestFilter;
    texture.minFilter = NearestFilter;
    texture.needsUpdate = true;
  }

  function disposeMaterial(material: Material): void {
    material.dispose();
  }

  function disposeModel(group: Group): void {
    group.traverse((child) => {
      if (!(child instanceof Mesh)) {
        return;
      }

      child.geometry.dispose();

      if (Array.isArray(child.material)) {
        child.material.forEach((material) => disposeMaterial(material));
      } else if (child.material) {
        disposeMaterial(child.material);
      }
    });
  }

  function normalizeModelOrigin(group: Group): Vector3 {
    const bounds = new Box3().setFromObject(group);
    if (bounds.isEmpty()) {
      return new Vector3(1, 2, 1);
    }

    const center = bounds.getCenter(new Vector3());
    const size = bounds.getSize(new Vector3());
    group.position.sub(center);
    return size;
  }

  function prepareMeshMaterials(group: Group): void {
    group.traverse((child) => {
      if (!(child instanceof Mesh)) {
        return;
      }

      if (Array.isArray(child.material)) {
        child.material = child.material.map((material) => material.clone());
      } else if (child.material) {
        child.material = child.material.clone();
      }

      const materials = Array.isArray(child.material)
        ? child.material
        : child.material
          ? [child.material]
          : [];

      materials.forEach((material) => {
        const target = material as Material & {
          transparent?: boolean;
          alphaTest?: number;
          side?: number;
          metalness?: number;
          roughness?: number;
          needsUpdate: boolean;
        };

        if ("transparent" in target) {
          target.transparent = true;
        }

        if ("alphaTest" in target) {
          target.alphaTest = 0.05;
        }

        if ("side" in target) {
          target.side = FrontSide;
        }

        if ("metalness" in target) {
          target.metalness = 0;
        }

        if ("roughness" in target) {
          target.roughness = 1;
        }

        target.needsUpdate = true;
      });
    });
  }

  function updateRotation(): void {
    if (!pivotGroup) {
      return;
    }

    pivotGroup.rotation.set(MathUtils.degToRad(-10), MathUtils.degToRad(yaw), 0);
  }

  function fitCameraToModel(): void {
    if (!camera) {
      return;
    }

    const safeScale = Math.max(0.4, scale);

    const verticalPadding = interactive ? 1.42 : 1.58;
    const horizontalPadding = interactive ? 1.48 : 1.7;

    const halfVerticalFov = MathUtils.degToRad(camera.fov * 0.5);
    const fitHeightDistance =
      (modelSize.y * verticalPadding * 0.5) / Math.tan(halfVerticalFov);

    const fitWidthDistance =
      (modelSize.x * horizontalPadding * 0.5) /
      (Math.tan(halfVerticalFov) * Math.max(camera.aspect, 0.1));

    let distance = Math.max(fitHeightDistance, fitWidthDistance, modelSize.z * 2.6);
    distance /= safeScale;

    camera.position.set(0, modelSize.y * 0.01, distance);
    camera.near = Math.max(0.01, distance / 120);
    camera.far = distance * 120;
    camera.lookAt(0, 0, 0);
    camera.updateProjectionMatrix();
  }

  function renderScene(): void {
    if (!scene || !camera || !renderer) {
      return;
    }

    renderer.render(scene, camera);
  }

  function updateRendererSize(): void {
    if (!renderer || !camera) {
      return;
    }

    const rect = hostElement.getBoundingClientRect();
    const width = Math.max(1, Math.floor(rect.width));
    const height = Math.max(1, Math.floor(rect.height));

    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.setSize(width, height, false);
    camera.aspect = width / height;

    fitCameraToModel();
    renderScene();
  }

  async function loadModelForArmType(): Promise<void> {
    if (!pivotGroup) {
      return;
    }

    const requestId = ++modelRequestId;
    const modelUrl = armType === "slim" ? alexModelUrl : steveModelUrl;

    try {
      const gltf = await gltfLoader.loadAsync(modelUrl);

      if (requestId !== modelRequestId || !pivotGroup) {
        disposeModel(gltf.scene);
        return;
      }

      if (modelGroup) {
        pivotGroup.remove(modelGroup);
        disposeModel(modelGroup);
        modelGroup = null;
      }

      modelGroup = gltf.scene;
      prepareMeshMaterials(modelGroup);
      modelSize = normalizeModelOrigin(modelGroup);
      pivotGroup.add(modelGroup);

      currentModelArmType = armType;

      fitCameraToModel();

      if (skinUrl.trim()) {
        await applySkinTexture(skinUrl);
      } else {
        renderScene();
      }
    } catch (error) {
      console.error("Failed to load GLB model for skin preview:", error);
    }
  }

  async function applySkinTexture(source: string): Promise<void> {
    if (!modelGroup) {
      return;
    }

    const normalizedSource = source.trim();
    if (!normalizedSource) {
      return;
    }

    const requestId = ++skinRequestId;

    try {
      const texture = await textureLoader.loadAsync(normalizedSource);

      if (requestId !== skinRequestId || !modelGroup) {
        texture.dispose();
        return;
      }

      setNearestFiltering(texture);

      if (currentSkinTexture) {
        currentSkinTexture.dispose();
      }
      currentSkinTexture = texture;

      modelGroup.traverse((child) => {
        if (!(child instanceof Mesh)) {
          return;
        }

        const materials = Array.isArray(child.material)
          ? child.material
          : child.material
            ? [child.material]
            : [];

        materials.forEach((material) => {
          const target = material as Material & {
            map?: Texture | null;
            transparent?: boolean;
            alphaTest?: number;
            side?: number;
            metalness?: number;
            roughness?: number;
            needsUpdate: boolean;
          };

          if ("map" in target) {
            target.map = texture;
          }

          if ("transparent" in target) {
            target.transparent = true;
          }

          if ("alphaTest" in target) {
            target.alphaTest = 0.05;
          }

          if ("side" in target) {
            target.side = FrontSide;
          }

          if ("metalness" in target) {
            target.metalness = 0;
          }

          if ("roughness" in target) {
            target.roughness = 1;
          }

          target.needsUpdate = true;
        });
      });

      currentSkinSource = normalizedSource;
      renderScene();
    } catch (error) {
      console.error("Failed to apply skin texture to GLB model:", error);
    }
  }

  function resetYaw(): void {
    yaw = initialYaw;
    updateRotation();
    renderScene();
  }

  function handlePointerDown(event: PointerEvent): void {
    if (!interactive) {
      return;
    }

    activePointerId = event.pointerId;
    dragStartX = event.clientX;
    dragStartYaw = yaw;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function handlePointerMove(event: PointerEvent): void {
    if (!interactive || activePointerId !== event.pointerId) {
      return;
    }

    yaw = dragStartYaw + (event.clientX - dragStartX) * 0.45;
    updateRotation();
    renderScene();
  }

  function releasePointer(event: PointerEvent): void {
    if (activePointerId !== event.pointerId) {
      return;
    }

    activePointerId = null;
    (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
  }

  onMount(() => {
    setupRenderer();
    updateRotation();
    updateRendererSize();

    resizeObserver = new ResizeObserver(() => {
      updateRendererSize();
    });
    resizeObserver.observe(hostElement);

    initialized = true;
    void loadModelForArmType();

    return () => {
      initialized = false;
      modelRequestId += 1;
      skinRequestId += 1;

      if (resizeObserver) {
        resizeObserver.disconnect();
        resizeObserver = null;
      }

      if (pivotGroup && modelGroup) {
        pivotGroup.remove(modelGroup);
        disposeModel(modelGroup);
        modelGroup = null;
      }

      if (currentSkinTexture) {
        currentSkinTexture.dispose();
        currentSkinTexture = null;
      }

      if (renderer) {
        renderer.dispose();
        renderer.domElement.remove();
        renderer = null;
      }

      scene = null;
      camera = null;
      pivotGroup = null;
      currentSkinSource = "";
      currentModelArmType = null;
    };
  });

  $: if (initialized && armType !== currentModelArmType) {
    void loadModelForArmType();
  }

  $: if (initialized && modelGroup && skinUrl.trim() && skinUrl.trim() !== currentSkinSource) {
    void applySkinTexture(skinUrl);
  }

  $: if (initialized) {
    const triggerScale = scale;
    const triggerInteractive = interactive;
    void triggerScale;
    void triggerInteractive;
    fitCameraToModel();
    renderScene();
  }

  $: if (initialYaw !== lastInitialYaw && activePointerId === null) {
    lastInitialYaw = initialYaw;
    yaw = initialYaw;
    if (initialized) {
      updateRotation();
      renderScene();
    }
  }
</script>

<div
  class="skin-preview"
  class:interactive
  class:dragging={activePointerId !== null}
  role={interactive ? "presentation" : undefined}
  on:pointerdown={handlePointerDown}
  on:pointermove={handlePointerMove}
  on:pointerup={releasePointer}
  on:pointercancel={releasePointer}
  on:dblclick={resetYaw}
>
  <div class="skin-preview-shadow" aria-hidden="true"></div>
  <div class="skin-preview-stage" bind:this={hostElement}></div>
</div>

<style>
  .skin-preview {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 0;
    user-select: none;
    touch-action: none;
  }

  .skin-preview.interactive {
    cursor: grab;
  }

  .skin-preview.interactive.dragging {
    cursor: grabbing;
  }

  .skin-preview-shadow {
    position: absolute;
    inset: auto 14% 7% 14%;
    height: 14%;
    border-radius: 50%;
    background: radial-gradient(
      circle,
      rgba(7, 11, 18, 0.4) 0%,
      rgba(7, 11, 18, 0.14) 54%,
      transparent 76%
    );
    filter: blur(12px);
    pointer-events: none;
  }

  .skin-preview-stage {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: inherit;
  }

  .skin-preview-stage :global(canvas.skin-preview-canvas) {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
