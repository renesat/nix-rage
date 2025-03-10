/* #include <config.h> */
#include <eval.hh>
#include <format>
#include <primops.hh>
#include <vector>

using namespace nix;

extern "C" {
char *nix_rage_decrypt(const char **identities, size_t size,
                       const char *filename, bool cache);
char *nix_rage_decrypt_error();
}

char *decrypt_content(EvalState &state, const PosIdx pos, Value **args) {
  state.forceList(
      *args[0], pos,
      "while evaluating the first argument passed to 'builtins.importAge'");
  state.forceValue(*args[1], pos);
  state.forceAttrs(
      *args[2], pos,
      "while evaluating the first argument passed to 'builtins.importAge'");

  if (args[1]->type() != nPath) {
    state
        .error<TypeError>("value is %1% while a path was expected",
                          showType(*args[1]))
        .atPos(pos)
        .debugThrow();
  }
  auto filename = const_cast<const char *>(args[1]->payload.path.path);

  std::vector<const char *> identities;
  identities.reserve(args[0]->listSize());
  for (auto elem : args[0]->listItems()) {
    state.forceValue(*elem, pos);
    if (elem->type() != nPath) {
      state
          .error<TypeError>("value is %1% while a path was expected",
                            showType(*elem))
          .atPos(pos)
          .debugThrow();
    }
    auto path = elem->payload.path.path;
    identities.push_back(const_cast<const char *>(path));
  }

  auto cache_value = args[2]->attrs()->get(state.symbols.create("cache"));
  bool cache = true;
  if (cache_value) {
    cache = cache_value->value->boolean();
  }
  auto content =
      nix_rage_decrypt(identities.data(), identities.size(), filename, cache);
  if (!content) {
    auto err = nix_rage_decrypt_error();
    if (!err) {
      throw Error("decrypt error while evaluation: unknown error");
    } else {
      throw Error(std::format("decrypt error while evaluation: {}", err));
    }
  };

  return content;
}

void prim_importAge(EvalState &state, const PosIdx pos, Value **args,
                    Value &v) {
  auto content = decrypt_content(state, pos, args);
  if (!content) {
    throw Error("decrypt error while evaluation");
  };

  Expr *parsed;
  try {
    parsed = state.parseExprFromString(std::move(content),
                                       state.rootPath(CanonPath::root));
  } catch (Error &e) {
    e.addTrace(state.positions[pos],
               "while parsing the output from 'builtins.importAge'");
    throw;
  }
  try {
    state.eval(parsed, v);
  } catch (Error &e) {
    e.addTrace(state.positions[pos],
               "while evaluating the output from 'builtins.importAge'");
    throw;
  }
}

void prim_readAgeFile(EvalState &state, const PosIdx pos, Value **args,
                      Value &v) {
  auto content = decrypt_content(state, pos, args);
  if (!content) {
    throw Error("decrypt error while evaluation");
  };
  v.mkString(content);
}

static std::vector<RegisterPrimOp> primops = std::vector{
    nix::RegisterPrimOp(nix::PrimOp{
        .name = "importAge",
        .args = {"identities", "path", "configs"},
        .arity = 3,
        .doc = "Import encypted .nix file",
        .fun = prim_importAge,
        .experimentalFeature = {},
    }),
    nix::RegisterPrimOp(nix::PrimOp{
        .name = "readAgeFile",
        .args = {"identities", "path", "configs"},
        .arity = 3,
        .doc = "Read encrypted file",
        .fun = prim_readAgeFile,
        .experimentalFeature = {},
    }),
};
