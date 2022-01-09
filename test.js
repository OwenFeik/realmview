window.onload = main

function main() {
    const canvas = document.createElement("canvas");
    document.body.appendChild(canvas);
    
    canvas.style.position = "absolute";
    canvas.style.top = canvas.style.left = "0";
    
    const fillWindow = () => {
        canvas.width = window.innerWidth;
        canvas.height = window.innerHeight;
    };
    window.onresize = fillWindow;
    fillWindow();
    
    const gl = canvas.getContext("webgl2");
    const programInfo = createProgram(gl);
    
    const input = document.createElement("input");
    input.type = "file";
    input.oninput = () => {
        const image = document.createElement("img");    
        image.onload = () => {
            const tex = {};
            tex.texture = gl.createTexture();
            gl.bindTexture(gl.TEXTURE_2D, tex.texture);
    
            gl.texParameteri(
                gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE
            );
            gl.texParameteri(
                gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE
            );
            gl.texParameteri(
                gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR
            );
            
            tex.width = image.naturalWidth;
            tex.height = image.naturalHeight;
    
            gl.texImage2D(
                gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, image
            );
    
            drawImage(gl, programInfo, tex);
        };

        const file_reader = new FileReader();
        file_reader.onloadend = () => {
            image.src = window.URL.createObjectURL(
                new Blob([file_reader.result])
            );
        };
        file_reader.readAsArrayBuffer(input.files[0]);
    };

    canvas.onclick = () => input.click();
}

function createShader(gl, source, type) {
    const shader = gl.createShader(type);
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
        console.log("Failed to compile shader.");
        console.log(gl.getShaderInfoLog(shader));
    }

    return shader;
}

const VERTEX_SHADER = `
attribute vec4 a_position;
attribute vec2 a_texcoord;

uniform mat4 u_matrix;

varying vec2 v_texcoord;

void main() {
    gl_Position = u_matrix * a_position;
    v_texcoord = a_texcoord;
}
`;

const FRAGMENT_SHADER = `
precision mediump float;

varying vec2 v_texcoord;

uniform sampler2D u_texture;

void main() {
    gl_FragColor = texture2D(u_texture, v_texcoord);
}
`;

function createProgram(gl) {
    const program = gl.createProgram();
    gl.attachShader(
        program, createShader(gl, VERTEX_SHADER, gl.VERTEX_SHADER)
    );
    gl.attachShader(
        program, createShader(gl, FRAGMENT_SHADER, gl.FRAGMENT_SHADER)
    );
    gl.linkProgram(program);
    
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
        console.log("Program creation failed");
        console.log(gl.getProgramInfoLog(program));
        return null;
    }
   
    const positionLocation = gl.getAttribLocation(program, "a_position");
    const texcoordLocation = gl.getAttribLocation(program, "a_texcoord");
    const positionBuffer = createBuffer(gl, [
        0, 0,
        0, 1,
        1, 0,
        1, 0,
        0, 1,
        1, 1
    ]);
    const texcoordBuffer = createBuffer(gl, [
        0, 0,
        0, 1,
        1, 0,
        1, 0,
        0, 1,
        1, 1
    ]);
    const matrixLocation = gl.getUniformLocation(program, "u_matrix");
    const textureLocation = gl.getUniformLocation(program, "u_texture");
    const programInfo = {
        program: program,
        positionLocation: positionLocation,
        texcoordLocation: texcoordLocation,
        positionBuffer: positionBuffer,
        texcoordBuffer: texcoordBuffer,
        matrixLocation: matrixLocation,
        textureLocation: textureLocation
    };

    return programInfo;
}

function createBuffer(gl, data) {
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(data), gl.STATIC_DRAW);
    return buffer;
}

function drawImage(gl, programInfo, tex) {
    gl.viewport(0, 0, gl.canvas.width, gl.canvas.height);
    gl.clear(gl.COLOR_BUFFER_BIT);

    gl.bindTexture(gl.TEXTURE_2D, tex.texture);
    gl.useProgram(programInfo.program);
    gl.bindBuffer(gl.ARRAY_BUFFER, programInfo.positionBuffer);
    gl.enableVertexAttribArray(programInfo.positionLocation);
    gl.vertexAttribPointer(
        programInfo.positionLocation, 2, gl.FLOAT, false, 0, 0
    );
    gl.bindBuffer(gl.ARRAY_BUFFER, programInfo.texcoordBuffer);
    gl.enableVertexAttribArray(programInfo.texcoordLocation);
    gl.vertexAttribPointer(
        programInfo.texcoordLocation, 2, gl.FLOAT, false, 0, 0
    );

    let m = m4.orthographic(0, gl.canvas.width, gl.canvas.height, 0, -1, 1);
    m = m4.scale(m, tex.width, tex.height, 1);

    console.log(m);

    gl.uniformMatrix4fv(programInfo.matrixLocation, false, m);
    gl.uniform1i(programInfo.textureLocation, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
}
